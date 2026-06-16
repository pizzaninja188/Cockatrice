//! Core rules processing (vanilla core ΓÇö simplified combat & mana).

use crate::custom::{self, ResolutionChoice, ResolutionCtx, ResolutionStep};
use crate::state::{
    AffectedScope, CombatState, ContinuousEffect, GameObject, GameState, ObjectId, OpeningSequence,
    PendingResolution, PendingTrigger, PlayerId, PlayerState, StackItem, TurnStep, Zone,
};
use prost::Message;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use thiserror::Error;
use tricerules_cards::mana::{ColorPip, ManaCost, ManaSymbol};
use tricerules_cards::primitives::{
    AbilityCost, CastTriggerPlayer, Color, ContinuousEffectKind, CounterKind, EffectDuration,
    Keyword, SpellEffectKind, SpellTypeFilter, TargetFilter, TargetKind, TokenController,
    TriggerCondition,
};
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

/// CR 702.8b: a card may be cast at instant speed if it is an instant or has flash. Sorceries
/// and flashless permanents remain sorcery-speed only.
fn castable_at_instant_speed(face: &tricerules_cards::FaceRef<'_>) -> bool {
    face.is_instant || face.keywords.contains(&tricerules_cards::Keyword::Flash)
}

pub(crate) fn shuffle_player_library(state: &mut GameState, player_idx: usize, mix: u64) {
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

/// Internal game events emitted at state-change sites to drive the unified trigger-collection pass
/// (CR 603.2). Each variant carries the minimum data needed to identify which triggers match.
enum GameEvent {
    EntersBattlefield {
        object_id: ObjectId,
    },
    /// `card_id` and `controller` must be captured before the zone move (object may be gone).
    Dies {
        object_id: ObjectId,
        card_id: String,
        controller: PlayerId,
    },
    Attacks {
        attacker_ids: Vec<ObjectId>,
    },
    CombatDamageToPlayer {
        attacker_id: ObjectId,
        defender_id: PlayerId,
    },
    UpkeepBegin,
    /// Fired when a spell is put on the stack (CR 601.2; triggers on cast, not resolution).
    SpellCast {
        caster: PlayerId,
        card_id: String,
    },
}

pub struct GameEngine {
    pub state: GameState,
    /// Shared process-wide registry (`CardRegistry::global()`); read-only.
    registry: &'static CardRegistry,
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
        let registry = CardRegistry::global();
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
                        deathtouch_damage: false,
                        counters: BTreeMap::new(),
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
            pending_triggers: VecDeque::new(),
            pending_resolution: None,
            continuous_effects: Vec::new(),
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
        let result = self.dispatch_command(player, cmd);
        // Replay determinism: `command_index` seeds shuffles (mulligan/search) and stamps
        // continuous-effect timestamps (CR 613.7). Only a command that is actually applied may
        // advance it — a rejected command must leave it untouched, otherwise replay (which
        // re-applies only the accepted commands) would compute different shuffles/timestamps and
        // diverge from live play.
        if result.is_ok() {
            self.state.command_index += 1;
        }
        result
    }

    /// Apply a single accepted command, mutating state and producing its event batch. Called by
    /// [`apply_command`], which owns the `command_index` bookkeeping and the pre-dispatch rejects.
    fn dispatch_command(
        &mut self,
        player: PlayerId,
        cmd: &RuledCommand,
    ) -> Result<RuledEventBatch, EngineError> {
        use rv1::ruled_command::Cmd;
        // CR 104.3a: a player may concede at any time. Handle it before the opening early-return so
        // a player can bail out of the choose-first / mulligan sequence.
        if matches!(cmd.cmd.as_ref(), Some(Cmd::Concede(_))) {
            return self.concede_batch(player);
        }
        if self.state.opening.is_some() {
            return self.apply_opening_command(player, cmd);
        }
        // A parked tier-3 custom resolution (CR 608) blocks every action but answering it
        // (or conceding), the same way a pending trigger gates the game.
        if self.state.pending_resolution.is_some()
            && !matches!(
                cmd.cmd.as_ref(),
                Some(Cmd::SubmitResolutionChoice(_)) | Some(Cmd::Concede(_))
            )
        {
            return Err(EngineError::Illegal(
                "resolve the pending choice before acting",
            ));
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
            Some(Cmd::CastSpell(cs)) => self.cast_spell(
                player,
                cs.hand_card_index as usize,
                &cs.targets,
                cs.x_value,
                &cs.flex_payments,
                cs.face_index as usize,
            ),
            Some(Cmd::ActivateAbility(aa)) => self.activate_ability(
                player,
                aa.permanent_id,
                aa.ability_index as usize,
                &aa.targets,
                &aa.flex_payments,
                aa.mana_option_index,
            ),
            Some(Cmd::ChooseTriggerTarget(ctt)) => {
                self.choose_trigger_target(player, ctt.target_object_id)
            }
            Some(Cmd::SubmitResolutionChoice(s)) => {
                self.submit_resolution_choice(player, &s.chosen_object_ids)
            }
            Some(Cmd::PlayLand(pl)) => self.play_land(player, pl.hand_card_index as usize),
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
                self.assign_combat_damage(acd.attacker_id, &pairs, acd.defending_player_damage)
            }
        };
        let mut b = res?;
        self.sweep_life();
        // SBAs are not checked while a tier-3 resolution is parked mid-resolution (CR 608/704);
        // they run when it completes. Zone view + legal actions still refresh so the deciding
        // player's client sees the drawn/revealed cards and the choice prompt.
        if self.state.pending_resolution.is_none() {
            let mut d = vec![];
            self.apply_sbas(&mut d)?;
            b.events.extend(d);
        }
        b.events.push(self.ev_zone_view_sync());
        // CR 106: emit each player's authoritative mana pool so the relay/clients mirror it onto
        // their mana-pool counters. An absolute snapshot per batch covers production (mana
        // abilities), payment (pay_mana), and emptying (clear_all_mana_pools on step/phase change)
        // in one place — clients never track mana locally, so a tampered client can't mint mana.
        for i in 0..self.state.players.len() {
            b.events.push(self.ev_mana_pool_updated(i));
        }
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
        if att.has_keyword(self.registry, Keyword::Flying)
            && !blk.has_keyword(self.registry, Keyword::Flying)
            && !blk.has_keyword(self.registry, Keyword::Reach)
        {
            return false;
        }

        // CR 702.13b — intimidate: can only be blocked by artifact creatures and/or
        // creatures that share a color with the intimidate creature.
        if att.has_keyword(self.registry, Keyword::Intimidate) {
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
                // CR 702.10b: Haste lets a creature attack (and use {T} abilities) even if it
                // just entered the battlefield this turn (i.e. ignore summoning sickness).
                let effectively_sick = o.summoning_sick
                    && !o.has_keyword(self.registry, tricerules_cards::Keyword::Haste);
                // CR 702.3b: defenders can't attack, so they don't count as eligible attackers.
                let has_defender =
                    o.has_keyword(self.registry, tricerules_cards::Keyword::Defender);
                o.zone == Zone::Battlefield
                    && o.owner == ap
                    && o.is_creature(self.registry)
                    && !effectively_sick
                    && !o.tapped
                    && !has_defender
            })
        })
    }

    fn defending_player_has_eligible_blockers(&self) -> bool {
        use tricerules_cards::Keyword;
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
        // Build the full list of untapped defender creatures up-front so the menace
        // check can count potential co-blockers without re-scanning the battlefield.
        let defenders: Vec<ObjectId> = self.state.players[dp_idx]
            .battlefield
            .iter()
            .filter(|&&oid| {
                self.state.objects.get(&oid).is_some_and(|o| {
                    o.zone == Zone::Battlefield
                        && o.owner == dp
                        && o.is_creature(self.registry)
                        && !o.tapped
                })
            })
            .copied()
            .collect();
        // A legal non-empty blocking assignment exists only when at least one defender
        // creature can participate in a valid block. For menace attackers (CR 702.111),
        // participation requires at least one OTHER defender that can block the same
        // attacker — otherwise the only achievable result is an illegal single-blocker.
        defenders.iter().any(|&cid| {
            attacking.iter().any(|&aid| {
                if !self.can_block(aid, cid) {
                    return false;
                }
                let has_menace = self
                    .state
                    .objects
                    .get(&aid)
                    .map(|o| o.has_keyword(self.registry, Keyword::Menace))
                    .unwrap_or(false);
                if has_menace {
                    // Need at least one other defender that can also block this attacker.
                    defenders
                        .iter()
                        .any(|&other| other != cid && self.can_block(aid, other))
                } else {
                    true
                }
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
            if !o.is_creature(self.registry) {
                return Err(EngineError::Illegal("not creature"));
            }
            // CR 702.3b: a creature with defender can't attack.
            if o.has_keyword(self.registry, tricerules_cards::Keyword::Defender) {
                return Err(EngineError::Illegal("creature has defender"));
            }
            // CR 702.10b: Haste bypasses summoning sickness — the creature may attack even
            // if it entered the battlefield this turn.
            let has_haste = o.has_keyword(self.registry, tricerules_cards::Keyword::Haste);
            if o.summoning_sick && !has_haste {
                return Err(EngineError::Illegal("summoning sick"));
            }
            if o.tapped {
                return Err(EngineError::Illegal("tapped"));
            }
            list.push(oid);
        }
        for &oid in &list {
            // CR 702.20b — Vigilance: attacking doesn't cause this creature to tap.
            let has_vigilance = self
                .state
                .objects
                .get(&oid)
                .map(|o| o.has_keyword(self.registry, tricerules_cards::Keyword::Vigilance))
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
            c.trample_player_damage.clear();
            c.damage_assignment_needed = false;
            c.assign_combat_damage_phase = false;
            c.attackers_declared = true;
            c.blockers_declared = false;
            c.first_strike_attackers.clear();
            c.first_strike_blockers.clear();
            c.first_strike_damage_done = false;
        } else {
            self.state.combat = Some(CombatState {
                attacking: list,
                blockers: HashMap::new(),
                damage_assignments: HashMap::new(),
                trample_player_damage: HashMap::new(),
                damage_assignment_needed: false,
                attackers_declared: true,
                blockers_declared: false,
                assign_combat_damage_phase: false,
                first_strike_attackers: Vec::new(),
                first_strike_blockers: HashMap::new(),
                first_strike_damage_done: false,
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
            .map(|&oid| object_display_name(&self.state, self.registry, oid))
            .collect();
        b.events.push(ev_log(format!(
            "P{} attacks with {}",
            ap,
            atk_names.join(", ")
        )));
        self.fire_triggers(
            GameEvent::Attacks {
                attacker_ids: attackers_for_event,
            },
            &mut b.events,
        );
        b.events.push(ev_priority_changed(self));
        Ok(b)
    }

    fn set_blockers(&mut self, pairs: &[rv1::BlockPair]) -> Result<RuledEventBatch, EngineError> {
        let defending_player = self
            .state
            .defending_player_id_1v1()
            .ok_or(EngineError::Illegal("defender missing"))?;
        // A blocker may appear at most once: CR 509.1a — a creature can only block one attacker.
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
            if !bobj.is_creature(self.registry) {
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
        // CR 702.111: menace — a creature with menace can't be blocked except by two or more
        // creatures. A menace creature with zero blockers is fine (it's unblocked); one blocker
        // is the illegal case. Return a prompt-friendly message so the UI can surface it.
        for (&att_id, blk_ids) in &attacker_to_blockers {
            if blk_ids.len() < 2 {
                let has_menace = self
                    .state
                    .objects
                    .get(&att_id)
                    .map(|o| o.has_keyword(self.registry, tricerules_cards::Keyword::Menace))
                    .unwrap_or(false);
                if has_menace {
                    return Err(EngineError::Illegal("Illegal blocks."));
                }
            }
        }
        // CR 702.19: trample attackers with 1+ blockers also require explicit damage assignment
        // (to split damage between blockers and the defending player).
        let damage_assignment_needed = {
            let objects = &self.state.objects;
            let registry = &self.registry;
            attacker_to_blockers.iter().any(|(atk_id, blks)| {
                let has_trample = objects
                    .get(atk_id)
                    .map(|o| o.has_keyword(registry, tricerules_cards::Keyword::Trample))
                    .unwrap_or(false);
                blks.len() > 1 || (blks.len() == 1 && has_trample)
            })
        };
        if let Some(c) = self.state.combat.as_mut() {
            c.blockers = attacker_to_blockers;
            c.damage_assignments.clear();
            c.trample_player_damage.clear();
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
                    let att = object_display_name(&self.state, self.registry, p.attacker_id);
                    let blk = object_display_name(&self.state, self.registry, p.blocker_id);
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
        player_damage: u32,
    ) -> Result<RuledEventBatch, EngineError> {
        // Phase 1: check gating conditions (immutable borrow, dropped at end of block).
        {
            let c = self
                .state
                .combat
                .as_ref()
                .ok_or(EngineError::Illegal("not in combat"))?;
            if !c.blockers_declared || !c.damage_assignment_needed || !c.assign_combat_damage_phase
            {
                return Err(EngineError::Illegal("combat damage assignment not open"));
            }
        }

        // Phase 2: compute trample flag and expected blockers before any borrow of combat.
        let att_has_trample = self
            .state
            .objects
            .get(&attacker_id)
            .map(|o| o.has_keyword(self.registry, tricerules_cards::Keyword::Trample))
            .unwrap_or(false);
        // CR 702.2b: any nonzero damage from a deathtouch source is lethal, which lowers the
        // per-blocker lethal amount the trample assignment must cover (CR 702.19e) to 1.
        let att_has_deathtouch = self
            .state
            .objects
            .get(&attacker_id)
            .map(|o| o.has_keyword(self.registry, tricerules_cards::Keyword::Deathtouch))
            .unwrap_or(false);

        // Clone expected blockers to free the immutable borrow on combat before the mutable one.
        let expected_blockers: Vec<ObjectId> = self
            .state
            .combat
            .as_ref()
            .and_then(|c| c.blockers.get(&attacker_id))
            .ok_or(EngineError::Illegal("attacker not blocked"))?
            .clone();

        if expected_blockers.len() < 2 && !att_has_trample {
            return Err(EngineError::Illegal("attacker not multiply-blocked"));
        }
        if expected_blockers.is_empty() {
            return Err(EngineError::Illegal(
                "cannot assign damage for unblocked attacker",
            ));
        }

        // Phase 3: validate assignment set (all expected blockers exactly once).
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
            .effective_power(attacker_id)
            .ok_or(EngineError::Illegal("attacker missing"))?;

        // Phase 4: validate damage amounts per trample rules.
        if att_has_trample {
            // CR 702.19b: must assign >= lethal damage to each blocker before sending excess to player.
            for &blk in &expected_blockers {
                let blk_toughness = self.effective_toughness(blk).unwrap_or(1);
                let marked = self.state.objects.get(&blk).map(|o| o.damage).unwrap_or(0);
                // CR 702.19e: with deathtouch, 1 damage counts as lethal for assignment, so the
                // attacker may assign just 1 to each blocker before trampling the rest over.
                let lethal = if att_has_deathtouch {
                    1
                } else {
                    blk_toughness.saturating_sub(marked).max(1)
                };
                let assigned = assignments
                    .iter()
                    .find(|(b, _)| *b == blk)
                    .map(|(_, d)| *d)
                    .unwrap_or(0);
                if assigned < lethal {
                    return Err(EngineError::Illegal(
                        "trample: must assign lethal damage to each blocker before assigning to player",
                    ));
                }
            }
            let blocker_sum: u32 = assignments.iter().map(|(_, d)| d).sum();
            if blocker_sum + player_damage != att_power {
                return Err(EngineError::Illegal(
                    "trample: total damage (blockers + player) must equal attacker power",
                ));
            }
        } else {
            if player_damage != 0 {
                return Err(EngineError::Illegal(
                    "cannot assign player damage without trample",
                ));
            }
            // CR 510.1c (post-2017): a multiply-blocked attacker no longer uses a declared
            // "damage assignment order" — the attacking player now freely divides the attacker's
            // combat damage among its blockers. So the only constraint here is that the assigned
            // amounts sum to the attacker's power; any per-blocker split is legal. (This is the
            // current rule, NOT a simplification — do not re-add an ordering/lethal-first check.)
            let sum: u32 = assignments.iter().map(|(_, d)| d).sum();
            if sum != att_power {
                return Err(EngineError::Illegal(
                    "assigned damage must equal attacker power",
                ));
            }
        }

        // Phase 5: store the assignment and check completion (mutable borrow).
        // Pre-compute which attackers need assignment to avoid borrowing self inside the closure.
        let needs_assignment: Vec<ObjectId> = {
            let objects = &self.state.objects;
            let registry = &self.registry;
            self.state
                .combat
                .as_ref()
                .unwrap()
                .blockers
                .iter()
                .filter_map(|(atk_id, blks)| {
                    let has_trample = objects
                        .get(atk_id)
                        .map(|o| o.has_keyword(registry, tricerules_cards::Keyword::Trample))
                        .unwrap_or(false);
                    if blks.len() > 1 || (blks.len() == 1 && has_trample) {
                        Some(*atk_id)
                    } else {
                        None
                    }
                })
                .collect()
        };

        let mut b = RuledEventBatch::default();
        let c = self.state.combat.as_mut().unwrap();
        c.damage_assignments
            .insert(attacker_id, assignments.to_vec());
        if att_has_trample && player_damage > 0 {
            c.trample_player_damage.insert(attacker_id, player_damage);
        }
        let all_done = needs_assignment
            .iter()
            .all(|atk| c.damage_assignments.contains_key(atk));
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
        let att_name = object_display_name(&self.state, self.registry, attacker_id);
        b.events
            .push(ev_log(format!("Combat damage assigned for {att_name}.")));

        if !self.state.combat.as_ref().unwrap().damage_assignment_needed {
            self.resolve_combat_damage_step(&mut b.events)?;
        } else {
            b.events.push(ev_priority_changed(self));
        }
        fill_legal(&mut b, self);
        Ok(b)
    }

    /// Resolve the current combat damage step (CR 510). Routes through the first-strike
    /// substep when any combatant has FirstStrike/DoubleStrike, then through the regular
    /// damage step. Emits phase labels, applies SBAs, and updates priority. Both call sites
    /// (the post-`assign_combat_damage` path and the `DeclareBlockers → CombatDamage` pass)
    /// go through this helper so the logic stays in one place.
    fn resolve_combat_damage_step(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        use tricerules_cards::Keyword;
        let ap = self.state.active_player_id();
        let c_init = self
            .state
            .combat
            .clone()
            .ok_or(EngineError::Illegal("combat?"))?;
        let needs_first_strike = !c_init.first_strike_damage_done
            && combat_needs_first_strike_step(&self.state, self.registry, &c_init);

        if needs_first_strike {
            // Snapshot which creatures had FS/DS at the start of the first-strike step. This is
            // the canonical CR 510.4 "participation list" used to exclude them from the regular
            // step (unless they have DoubleStrike).
            let registry = &self.registry;
            let objects = &self.state.objects;
            let is_fs_or_ds = |id: ObjectId| {
                objects.get(&id).is_some_and(|o| {
                    o.has_keyword(registry, Keyword::FirstStrike)
                        || o.has_keyword(registry, Keyword::DoubleStrike)
                })
            };
            let fs_attackers: Vec<ObjectId> = c_init
                .attacking
                .iter()
                .copied()
                .filter(|&id| is_fs_or_ds(id))
                .collect();
            let fs_blockers: HashMap<ObjectId, Vec<ObjectId>> = c_init
                .blockers
                .iter()
                .map(|(att, bs)| {
                    (
                        *att,
                        bs.iter().copied().filter(|&id| is_fs_or_ds(id)).collect(),
                    )
                })
                .collect();
            if let Some(cc) = self.state.combat.as_mut() {
                cc.first_strike_attackers = fs_attackers;
                cc.first_strike_blockers = fs_blockers;
                cc.first_strike_damage_done = true;
            }
            let c2 = self
                .state
                .combat
                .clone()
                .ok_or(EngineError::Illegal("combat?"))?;
            // Emit PhaseChanged before resolving damage so the C++ client clears its
            // stack-object set before any combat damage triggers are pushed (StackPushed).
            // This mirrors adv_on_empty_stack(Untap) which emits PhaseChanged first, then
            // fires upkeep triggers — ensuring players see the non-empty stack and are not
            // auto-passed through triggered abilities.
            self.clear_all_mana_pools();
            self.state.turn_step = TurnStep::FirstStrikeDamage;
            if let Some(i) = self.state.player_idx(ap) {
                self.state.priority_idx = i;
            }
            self.state.passes_since_stack_change = 0;
            events.push(ev_log("First strike combat damage dealt.".to_string()));
            events.push(ev_phase_labeled(self, "first_strike_damage"));
            self.resolve_combat_damage(&c2, DamagePass::FirstStrike, events)?;
            // CR 510.2 + 704: SBAs run between damage steps so creatures with lethal damage are
            // moved to graveyards before the regular step decides who deals damage.
            self.apply_sbas(events)?;
            let legend_events = self.apply_legend_sbas()?;
            events.extend(legend_events);
            events.push(ev_priority_changed(self));
        } else {
            // Emit PhaseChanged before resolving damage so the C++ client clears its
            // stack-object set before any combat damage triggers are pushed (StackPushed).
            self.state.combat = None;
            self.clear_all_mana_pools();
            self.state.turn_step = TurnStep::CombatDamage;
            if let Some(i) = self.state.player_idx(ap) {
                self.state.priority_idx = i;
            }
            self.state.passes_since_stack_change = 0;
            events.push(ev_log("Combat damage dealt.".to_string()));
            events.push(ev_phase_labeled(self, "combat_damage"));
            self.resolve_combat_damage(&c_init, DamagePass::Normal, events)?;
            let legend_events = self.apply_legend_sbas()?;
            events.extend(legend_events);
            events.push(ev_priority_changed(self));
        }
        Ok(())
    }

    fn resolve_combat_damage(
        &mut self,
        c: &CombatState,
        pass: DamagePass,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        use tricerules_cards::Keyword;
        let dfd = self.state.defending_player_id_1v1().unwrap();
        let ap = self.state.active_player_id();
        let mut total_life_lost: i32 = 0;
        // (controller_id, amount) pairs — collected during damage assignment, applied after.
        let mut lifelink_gains: Vec<(PlayerId, u32)> = Vec::new();
        // (attacker_id, defending_player_id) — collected for combat-damage-to-player triggers.
        let mut combat_dmg_to_player: Vec<(ObjectId, PlayerId)> = Vec::new();

        // CR 510.4 ASSIGNMENT rule: in the first-strike pass, only creatures with FirstStrike
        // or DoubleStrike assign damage; in the regular pass, creatures that did NOT assign
        // in the first-strike pass do, plus those that have DoubleStrike. Crucially, creatures
        // RECEIVE damage normally regardless of *their own* participation — a vanilla blocker
        // can be killed by a first-strike attacker before it ever swings, and a vanilla blocker
        // still deals damage back to a first-strike attacker in the regular step. We therefore
        // iterate over ALL attackers and gate each damage direction independently:
        //   - "attacker deals damage" -> attacker's participation
        //   - "blocker deals damage" -> blocker's participation
        // When no first-strike step occurred (`c.first_strike_attackers` empty), every creature
        // participates in the regular pass (vanilla combat).

        for &att in &c.attacking {
            if self.state.objects.get(&att).map(|a| a.zone) != Some(Zone::Battlefield) {
                continue;
            }
            let attacker_participates =
                object_participates_in_pass(&self.state, self.registry, c, pass, att, true);
            // Capture attacker properties before any mutation.
            let att_power = self.effective_power(att).unwrap_or(0);
            let att_has_lifelink = self
                .state
                .objects
                .get(&att)
                .map(|o| o.has_keyword(self.registry, Keyword::Lifelink))
                .unwrap_or(false);
            let att_has_deathtouch = self
                .state
                .objects
                .get(&att)
                .map(|o| o.has_keyword(self.registry, Keyword::Deathtouch))
                .unwrap_or(false);
            let att_owner = self.state.objects.get(&att).map(|o| o.owner).unwrap_or(ap);
            let att_has_trample = self
                .state
                .objects
                .get(&att)
                .map(|o| o.has_keyword(self.registry, Keyword::Trample))
                .unwrap_or(false);

            let blockers = c.blockers.get(&att).map(|v| v.as_slice()).unwrap_or(&[]);

            if blockers.is_empty() {
                // Unblocked: deal full power to defending player — only if the attacker assigns
                // damage this pass (CR 510.4).
                if attacker_participates {
                    let p = att_power as i32;
                    if let Some(di) = self.state.player_idx(dfd) {
                        self.state.players[di].life -= p;
                        total_life_lost += p;
                    }
                    if att_power > 0 {
                        combat_dmg_to_player.push((att, dfd));
                    }
                    // CR 702.15b: attacker with lifelink causes its controller to gain that much life.
                    if att_has_lifelink && att_power > 0 {
                        lifelink_gains.push((att_owner, att_power));
                    }
                }
            } else if blockers.len() == 1 && !att_has_trample {
                // Single blocker, no trample: exchange power. The attacker always deals damage to
                // its sole blocker (since we're in the attacker's participation loop), but the
                // blocker only deals damage back if it participates in this pass (CR 510.4).
                let blk = blockers[0];
                let blocker_participates =
                    object_participates_in_pass(&self.state, self.registry, c, pass, blk, false)
                        && self.state.objects.get(&blk).map(|o| o.zone) == Some(Zone::Battlefield);
                let bpw = self.effective_power(blk).unwrap_or(0);
                let blk_has_lifelink = self
                    .state
                    .objects
                    .get(&blk)
                    .map(|o| o.has_keyword(self.registry, Keyword::Lifelink))
                    .unwrap_or(false);
                let blk_has_deathtouch = self
                    .state
                    .objects
                    .get(&blk)
                    .map(|o| o.has_keyword(self.registry, Keyword::Deathtouch))
                    .unwrap_or(false);
                let blk_owner = self.state.objects.get(&blk).map(|o| o.owner).unwrap_or(dfd);
                if blocker_participates {
                    if let Some(af) = self.state.objects.get_mut(&att) {
                        af.damage += bpw;
                        // CR 702.2b / CR 704.5h: any damage from a deathtouch source is lethal.
                        if blk_has_deathtouch && bpw > 0 {
                            af.deathtouch_damage = true;
                        }
                    }
                }
                if attacker_participates {
                    if let Some(bf) = self.state.objects.get_mut(&blk) {
                        bf.damage += att_power;
                        // CR 702.2b: any damage from attacker with deathtouch is lethal.
                        if att_has_deathtouch && att_power > 0 {
                            bf.deathtouch_damage = true;
                        }
                    }
                    // CR 702.15b: attacker with lifelink gains life = damage dealt to blocker.
                    if att_has_lifelink && att_power > 0 {
                        lifelink_gains.push((att_owner, att_power));
                    }
                }
                // CR 702.15b: blocker with lifelink gains life = damage dealt to attacker.
                if blocker_participates && blk_has_lifelink && bpw > 0 {
                    lifelink_gains.push((blk_owner, bpw));
                }
            } else {
                // Multiple blockers OR single-blocker with trample: all blockers deal their power
                // to the attacker simultaneously; active player assigns how the attacker's combat
                // damage is divided among blockers (and, for trample, the defending player).
                // CR 510.4: in a given damage step, only participating blockers deal damage back.
                // Tuple: (id, power, has_lifelink, has_deathtouch, owner, participates)
                let blocker_info: Vec<(ObjectId, u32, bool, bool, PlayerId, bool)> = blockers
                    .iter()
                    .map(|&blk| {
                        let pw = self.effective_power(blk).unwrap_or(0);
                        let has_ll = self
                            .state
                            .objects
                            .get(&blk)
                            .map(|o| o.has_keyword(self.registry, Keyword::Lifelink))
                            .unwrap_or(false);
                        let has_dt = self
                            .state
                            .objects
                            .get(&blk)
                            .map(|o| o.has_keyword(self.registry, Keyword::Deathtouch))
                            .unwrap_or(false);
                        let owner = self.state.objects.get(&blk).map(|o| o.owner).unwrap_or(dfd);
                        let participates = object_participates_in_pass(
                            &self.state,
                            self.registry,
                            c,
                            pass,
                            blk,
                            false,
                        ) && self.state.objects.get(&blk).map(|o| o.zone)
                            == Some(Zone::Battlefield);
                        (blk, pw, has_ll, has_dt, owner, participates)
                    })
                    .collect();
                let total_blocker_power: u32 = blocker_info
                    .iter()
                    .filter(|(_, _, _, _, _, p)| *p)
                    .map(|(_, pw, _, _, _, _)| pw)
                    .sum();
                // CR 702.2b: if any participating blocker has deathtouch and dealt damage,
                // mark the attacker.
                let any_blocker_deathtouch_hit = blocker_info
                    .iter()
                    .any(|(_, pw, _, has_dt, _, p)| *p && *has_dt && *pw > 0);
                if let Some(af) = self.state.objects.get_mut(&att) {
                    af.damage += total_blocker_power;
                    if any_blocker_deathtouch_hit {
                        af.deathtouch_damage = true;
                    }
                }
                // The attacker assigns damage to its blockers only on a pass it participates in
                // (CR 510.4). On the off pass, blockers still deal damage back (handled above).
                if attacker_participates {
                    let pairs = c.damage_assignments.get(&att).ok_or(EngineError::Illegal(
                        "combat damage assignments missing for multiply-blocked attacker",
                    ))?;
                    for &(blk, dmg) in pairs {
                        if let Some(bf) = self.state.objects.get_mut(&blk) {
                            bf.damage += dmg;
                            // CR 702.2b: any damage from attacker with deathtouch is lethal.
                            if att_has_deathtouch && dmg > 0 {
                                bf.deathtouch_damage = true;
                            }
                        }
                    }
                    // CR 702.19: deal trample excess damage to the defending player.
                    let player_trample_dmg =
                        c.trample_player_damage.get(&att).copied().unwrap_or(0);
                    if player_trample_dmg > 0 {
                        if let Some(di) = self.state.player_idx(dfd) {
                            self.state.players[di].life -= player_trample_dmg as i32;
                            total_life_lost += player_trample_dmg as i32;
                        }
                        // CR 510: trample excess is combat damage the attacker deals to the
                        // defending player, so it fires "deals combat damage to a player" triggers
                        // exactly like an unblocked hit.
                        combat_dmg_to_player.push((att, dfd));
                    }
                    // CR 702.15b: attacker with lifelink gains life = damage dealt to all blockers.
                    if att_has_lifelink && att_power > 0 {
                        lifelink_gains.push((att_owner, att_power));
                    }
                }
                // CR 702.15b: each participating blocker with lifelink gains life = damage it dealt
                // to the attacker.
                for (_, blk_pw, blk_has_ll, _, blk_owner, blk_participates) in blocker_info {
                    if blk_participates && blk_has_ll && blk_pw > 0 {
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
        for (att_id, def_id) in combat_dmg_to_player {
            self.fire_triggers(
                GameEvent::CombatDamageToPlayer {
                    attacker_id: att_id,
                    defender_id: def_id,
                },
                events,
            );
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
        if !self.state.pending_triggers.is_empty() {
            return Err(EngineError::Illegal(
                "must choose trigger target before passing priority",
            ));
        }
        if self.state.pending_resolution.is_some() {
            return Err(EngineError::Illegal(
                "must submit resolution choice before passing priority",
            ));
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
        // A tier-3 custom resolution may have parked mid-resolution awaiting a player choice
        // (CR 608): no player holds priority then — the ResolutionChoiceRequired event already
        // drives the deciding player — so don't advance priority or run SBAs until it completes.
        if self.state.pending_resolution.is_none() {
            ev.push(ev_priority_changed(self));
            self.apply_sbas(&mut ev)?;
        }
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
                self.fire_triggers(GameEvent::UpkeepBegin, ev);
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
                        trample_player_damage: HashMap::new(),
                        damage_assignment_needed: false,
                        attackers_declared: false,
                        blockers_declared: false,
                        assign_combat_damage_phase: false,
                        first_strike_attackers: Vec::new(),
                        first_strike_blockers: HashMap::new(),
                        first_strike_damage_done: false,
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
                let objects = &self.state.objects;
                let registry = &self.registry;
                let multiblock_missing = c.blockers.iter().any(|(atk, blks)| {
                    // Trample with 1+ blockers also requires explicit damage assignment (CR 702.19).
                    let has_trample = objects
                        .get(atk)
                        .map(|o| o.has_keyword(registry, tricerules_cards::Keyword::Trample))
                        .unwrap_or(false);
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

    /// Effective (rules-visible) power of `oid`: base from card definition, then CR 613.4
    /// layer 7c modifying continuous effects, then layer 7d +1/+1 / -1/-1 counters. Returns
    /// `None` for non-creatures (no base power).
    pub fn effective_power(&self, oid: ObjectId) -> Option<u32> {
        let obj = self.state.objects.get(&oid)?;
        let base = obj.power? as i32;
        let delta: i32 = self
            .state
            .continuous_effects
            .iter()
            .filter(|e| e.affects(oid))
            .map(|e| match &e.kind {
                ContinuousEffectKind::PtModify { delta_power, .. } => *delta_power,
            })
            .sum();
        // Layer 7d: counters apply after all layer-7c modifying effects (CR 613.4d).
        Some((base + delta + obj.counter_pt_delta()).max(0) as u32)
    }

    /// Effective (rules-visible) toughness of `oid`: base, then layer-7c continuous effects,
    /// then layer-7d counters (CR 613.4).
    pub fn effective_toughness(&self, oid: ObjectId) -> Option<u32> {
        let obj = self.state.objects.get(&oid)?;
        let base = obj.toughness? as i32;
        let delta: i32 = self
            .state
            .continuous_effects
            .iter()
            .filter(|e| e.affects(oid))
            .map(|e| match &e.kind {
                ContinuousEffectKind::PtModify {
                    delta_toughness, ..
                } => *delta_toughness,
            })
            .sum();
        // Layer 7d: counters apply after all layer-7c modifying effects (CR 613.4d).
        Some((base + delta + obj.counter_pt_delta()).max(0) as u32)
    }

    /// CR 514.2: drain all UntilEndOfTurn continuous effects. Called from
    /// `finish_cleanup_roll_new_turn`, after CR 514.1 discards have already completed.
    /// `GameObject.power/toughness` hold printed base values and never need resetting.
    fn cleanup_until_end_of_turn_creature_pt(&mut self) {
        self.state
            .continuous_effects
            .retain(|e| e.duration != EffectDuration::UntilEndOfTurn);
    }

    /// CR 514.2: damage marked on permanents is removed during cleanup.
    fn cleanup_marked_damage(&mut self) {
        for o in self.state.objects.values_mut() {
            if o.zone == Zone::Battlefield && (o.damage != 0 || o.deathtouch_damage) {
                o.damage = 0;
                o.deathtouch_damage = false;
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

        // Abilities — and spell copies (CR 707.10d) — leave no object behind when they resolve;
        // only a genuinely cast spell has a backing card that moves to a zone. A copy has no
        // `GameObject` in `objects`, so it must take the same no-zone-move path as an ability.
        let is_ability = top.ability_text.is_some();
        let leaves_no_object = is_ability || top.is_copy;
        if leaves_no_object {
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                    object_id: top.id,
                    // Abilities cease to exist on resolution; graveyard tells the C++ server
                    // not to expect a permanent to land.
                    destination: rv1::StackResolveDestination::Graveyard as i32,
                })),
            });
        } else {
            // CR 709/712/715: permanence is the *cast face's* (Ice resolves to graveyard; an MDFC
            // permanent face resolves to the battlefield as that face).
            let resolves_to_battlefield = self
                .registry
                .get(&card_id)
                .and_then(|d| d.face(top.face_index))
                .map(|f| f.is_permanent())
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
            if resolves_to_battlefield {
                self.fire_triggers(GameEvent::EntersBattlefield { object_id: top.id }, events);
            }
        }

        // Determine effects: for spells use spell_effect (Vec); for abilities wrap the single
        // effect. Triggered and activated abilities are now uniform — both carry a plain
        // `SpellEffectKind` (self-referencing effects use a `Self_` target filter, bound below).
        let (effects, spell_label): (Vec<SpellEffectKind>, String) = if is_ability {
            let ability_index = top.ability_index.unwrap_or(0);
            let def = self.registry.get(&card_id);
            let name = def
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "Ability".into());
            let abilities = if top.is_triggered {
                def.map(|d| &d.triggered_abilities[..])
                    .and_then(|a| a.get(ability_index))
                    .map(|a| a.effect.clone())
            } else {
                def.and_then(|d| d.activated_abilities.get(ability_index))
                    .map(|a| a.effect.clone())
            };
            (vec![abilities.unwrap_or(SpellEffectKind::None)], name)
        } else {
            // CR 709/712/715: resolve the cast face's effects and show its name.
            let face = self
                .registry
                .get(&card_id)
                .and_then(|d| d.face(top.face_index));
            let effects = face.map(|f| f.spell_effect.to_vec()).unwrap_or_default();
            let name = face
                .map(|f| f.name.to_string())
                .unwrap_or_else(|| "Spell".into());
            (effects, name)
        };

        // Tier-3 (CR 608): a custom effect owns this spell's resolution. The spell card has
        // already moved to its zone (graveyard/battlefield above); hand off the algorithm to the
        // registered `CardEffect`, which either completes now or parks awaiting a player choice.
        // A copy is excluded: the resumable custom machinery (`begin_custom_resolution`) expects the
        // spell's backing `GameObject`, which a copy lacks. Copying a tier-3 spell is a documented
        // limitation (the copy resolves its non-custom effects only, if any).
        if !is_ability && !top.is_copy {
            let custom_key = self
                .registry
                .get(&card_id)
                .and_then(|d| d.face(top.face_index))
                .and_then(|f| f.custom_effect.map(str::to_string));
            if let Some(custom_key) = custom_key {
                return self.begin_custom_resolution(top, custom_key, events);
            }
        }

        let fizzle = spell_has_no_legal_targets_at_resolution(
            &self.state,
            self.registry,
            &effects,
            &targets,
            controller,
        );
        if fizzle {
            events.push(ev_log(format!("{spell_label} fizzles (no legal targets).")));
            return Ok(());
        }

        for effect in effects {
            match effect {
                SpellEffectKind::DamageTarget { amount, .. } => {
                    // CR 107.3: `amount` may be the cast-time X (Fireball) or a literal (Bolt).
                    let amount = amount.resolve(top.chosen_x);
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
                            let tgt = object_display_name(&self.state, self.registry, tid);
                            if let Some(t) = self.state.objects.get_mut(&tid) {
                                if t.zone == Zone::Battlefield && t.is_creature(self.registry) {
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
                    // Blue Sun's Zenith / Braingeyser: `count` may be the cast-time X.
                    let count = count.resolve(top.chosen_x);
                    let idx = self.state.player_idx(controller).unwrap();
                    // CR 120.3 / 104.3c: drawing from an empty library does NOT fail the spell —
                    // draw as many as possible, then the player loses as a state-based action
                    // (swept in by `sweep_life`). Aborting resolution here would corrupt state
                    // (cards already drawn, stack already popped).
                    let mut drawn = 0u32;
                    let mut decked_out = false;
                    for _ in 0..count {
                        if self.state.players[idx].library.is_empty() {
                            decked_out = true;
                            break;
                        }
                        draw_card(&mut self.state.players[idx], &mut self.state.objects)?;
                        drawn += 1;
                    }
                    let noun = if drawn == 1 { "card" } else { "cards" };
                    events.push(ev_log(format!(
                        "P{controller} draws {drawn} {noun} ({spell_label})."
                    )));
                    if decked_out {
                        self.state.players[idx].has_lost = true;
                        events.push(ev_log(format!(
                            "P{controller} tried to draw from an empty library and loses (CR 104.3c)."
                        )));
                    }
                }
                SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    target,
                } => {
                    // `Self_` is auto-bound to the source permanent (CR 115 — not a chosen target);
                    // every other filter uses the player's selected target.
                    let tid = if matches!(target.kind, TargetKind::Self_) {
                        top.source_permanent_id
                    } else {
                        targets.first().copied()
                    };
                    if let Some(tid) = tid {
                        let is_valid_target = self
                            .state
                            .objects
                            .get(&tid)
                            .map(|t| t.zone == Zone::Battlefield && t.is_creature(self.registry))
                            .unwrap_or(false);
                        if is_valid_target {
                            let tgt = object_display_name(&self.state, self.registry, tid);
                            self.state.continuous_effects.push(ContinuousEffect {
                                source_id: top.source_permanent_id,
                                affected: AffectedScope::Single(tid),
                                kind: ContinuousEffectKind::PtModify {
                                    delta_power: power,
                                    delta_toughness: toughness,
                                },
                                duration: EffectDuration::UntilEndOfTurn,
                                timestamp: self.state.command_index,
                            });
                            events.push(ev_log(format!(
                                "{spell_label} gives +{power}/+{toughness} to {tgt}"
                            )));
                        }
                    }
                }
                SpellEffectKind::PutCounters {
                    counter,
                    count,
                    target,
                } => {
                    // `Self_` is auto-bound to the source permanent (CR 115); any other filter
                    // uses the chosen target. Counters go on a permanent on the battlefield.
                    let tid = if matches!(target.kind, TargetKind::Self_) {
                        top.source_permanent_id
                    } else {
                        targets.first().copied()
                    };
                    if let Some(tid) = tid {
                        let is_valid_target = self
                            .state
                            .objects
                            .get(&tid)
                            .map(|t| t.zone == Zone::Battlefield && t.is_creature(self.registry))
                            .unwrap_or(false);
                        if is_valid_target {
                            let tgt = object_display_name(&self.state, self.registry, tid);
                            if let Some(t) = self.state.objects.get_mut(&tid) {
                                *t.counters.entry(counter).or_insert(0) += count;
                            }
                            events.push(ev_log(format!(
                                "{spell_label} puts {count} {} counter{} on {tgt}",
                                counter_label(counter),
                                if count == 1 { "" } else { "s" },
                            )));
                            // Annihilation / toughness-0 death are checked by the SBA pass that
                            // runs after this resolution (CR 122.3, CR 704.5f).
                        }
                    }
                }
                SpellEffectKind::DestroyTarget { .. } => {
                    if let Some(&tid) = targets.first() {
                        let tgt = object_display_name(&self.state, self.registry, tid);
                        let indestructible = self
                            .state
                            .objects
                            .get(&tid)
                            .map(|o| o.has_keyword(self.registry, Keyword::Indestructible))
                            .unwrap_or(false);
                        if indestructible {
                            events.push(ev_log(format!(
                                "{spell_label} has no effect: {tgt} is indestructible."
                            )));
                        } else {
                            events.push(ev_log(format!("{spell_label} destroys {tgt}")));
                            let owner = self.state.objects.get(&tid).map(|o| o.owner);
                            let card_id_t = self.state.objects.get(&tid).map(|o| o.card_id.clone());
                            destroy_permanent(&mut self.state, tid)?;
                            if let Some(owner_id) = owner {
                                events.push(permanent_moved_event(
                                    &self.state,
                                    tid,
                                    owner_id,
                                    rv1::permanent_moved::Destination::Graveyard,
                                ));
                            }
                            if let (Some(cid), Some(ctrl)) = (card_id_t, owner) {
                                self.fire_triggers(
                                    GameEvent::Dies {
                                        object_id: tid,
                                        card_id: cid,
                                        controller: ctrl,
                                    },
                                    events,
                                );
                            }
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
                            // CR 707.10d: a copy of a spell has no backing card — it simply ceases
                            // to exist when it leaves the stack. Only a genuinely cast spell has a
                            // `GameObject` that moves; CR 701.6a sends that card to its OWNER's
                            // graveyard via an explicit PermanentMoved so the C++ relay routes the
                            // physical card off the shared stack. Moving a copy would error on the
                            // missing object and corrupt the already-popped stack.
                            if !st.is_copy {
                                let owner = self.state.objects.get(&st.id).map(|o| o.owner);
                                move_object_to_zone(&mut self.state, st.id, Zone::Graveyard)?;
                                if let Some(owner) = owner {
                                    events.push(permanent_moved_event(
                                        &self.state,
                                        st.id,
                                        owner,
                                        rv1::permanent_moved::Destination::Graveyard,
                                    ));
                                }
                            }
                            events.push(ev_log(format!("{spell_label} counters {tgt}")));
                        }
                    }
                }
                SpellEffectKind::CopyTargetSpell { count } => {
                    // CR 707.10: create `count` copies of the target spell on the stack, each
                    // controlled by this spell's controller. The copy is not cast (no mana/cast
                    // triggers) and ceases to exist after resolving (handled by `is_copy`). It uses
                    // the original's chosen X, face, and targets — CR 707.10c new-target choice is
                    // deferred (the copy keeps the original's targets). If the target spell has left
                    // the stack (e.g. countered in response), this fizzles via the resolution check.
                    if let Some(&tid) = targets.first() {
                        if let Some(src) = self.state.stack.iter().find(|s| s.id == tid).cloned() {
                            let copied_name = self
                                .registry
                                .get(&src.card_id)
                                .and_then(|d| d.face(src.face_index))
                                .map(|f| f.name.to_string())
                                .unwrap_or_else(|| src.card_id.clone());
                            for _ in 0..count {
                                let copy_id = self.state.next_object_id;
                                self.state.next_object_id += 1;
                                self.state.stack.push(StackItem {
                                    id: copy_id,
                                    controller,
                                    card_id: src.card_id.clone(),
                                    targets: src.targets.clone(),
                                    ability_text: None,
                                    source_permanent_id: None,
                                    ability_index: None,
                                    is_triggered: false,
                                    is_copy: true,
                                    chosen_x: src.chosen_x,
                                    face_index: src.face_index,
                                });
                                events.push(rv1::RuledEvent {
                                    ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                                        object_id: copy_id,
                                        description: copied_name.clone(),
                                        targets: src
                                            .targets
                                            .iter()
                                            .map(|&o| rv1::TargetRef { object_id: o })
                                            .collect(),
                                        // Overlay marks the stack item as a copy in the existing UI.
                                        ability_annotation: "(copy)".to_string(),
                                        card_id: src.card_id.clone(),
                                        is_copy: true,
                                    })),
                                });
                                events.push(ev_log(format!(
                                    "{spell_label} copies {copied_name} (P{controller})"
                                )));
                            }
                        }
                    }
                }
                SpellEffectKind::GainLife { amount } => {
                    let amount = amount.resolve(top.chosen_x);
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
                        let tgt = object_display_name(&self.state, self.registry, tid);
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
                        let tgt = object_display_name(&self.state, self.registry, tid);
                        // CR 608: read effective power at resolution before the object leaves.
                        let power = self.effective_power(tid).unwrap_or(0);
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
                                events.push(ev_log(format!(
                                    "P{target_controller} gains {power} life."
                                )));
                            }
                        }
                    }
                }
                SpellEffectKind::ReturnTargetCreatureToHand
                | SpellEffectKind::ReturnTargetPermanentToHand => {
                    if let Some(&tid) = targets.first() {
                        let tgt = object_display_name(&self.state, self.registry, tid);
                        let owner = self.state.objects.get(&tid).map(|o| o.owner);
                        move_object_to_zone(&mut self.state, tid, Zone::Hand)?;
                        // Reset transient permanent state when leaving the battlefield.
                        if let Some(o) = self.state.objects.get_mut(&tid) {
                            o.tapped = false;
                            o.summoning_sick = false;
                            o.damage = 0;
                            o.deathtouch_damage = false;
                            // power/toughness hold the printed base and are not cleared:
                            // they remain valid for when the card re-enters the battlefield.
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
                SpellEffectKind::TapTarget { .. } => {
                    if let Some(&tid) = targets.first() {
                        let tgt = object_display_name(&self.state, self.registry, tid);
                        if let Some(o) = self.state.objects.get_mut(&tid) {
                            if o.zone == Zone::Battlefield && !o.tapped {
                                o.tapped = true;
                                events.push(ev_log(format!("{spell_label} taps {tgt}")));
                            }
                        }
                    }
                }
                SpellEffectKind::DestroyAll { kind } => {
                    // CR 701.7 / 704.4: all matching permanents are destroyed simultaneously,
                    // then their "dies" triggers fire together. Indestructible permanents survive
                    // (CR 702.12b). Untargeted, so hexproof/shroud are irrelevant.
                    let victims = battlefield_objects_matching(&self.state, self.registry, &kind);
                    let mut destroyed: Vec<(ObjectId, String, PlayerId)> = Vec::new();
                    for tid in victims {
                        let indestructible = self
                            .state
                            .objects
                            .get(&tid)
                            .map(|o| o.has_keyword(self.registry, Keyword::Indestructible))
                            .unwrap_or(false);
                        let tgt = object_display_name(&self.state, self.registry, tid);
                        if indestructible {
                            events.push(ev_log(format!(
                                "{tgt} is indestructible and survives {spell_label}."
                            )));
                            continue;
                        }
                        let owner = self.state.objects.get(&tid).map(|o| o.owner);
                        let card_id_t = self.state.objects.get(&tid).map(|o| o.card_id.clone());
                        destroy_permanent(&mut self.state, tid)?;
                        events.push(ev_log(format!("{spell_label} destroys {tgt}")));
                        if let Some(owner_id) = owner {
                            events.push(permanent_moved_event(
                                &self.state,
                                tid,
                                owner_id,
                                rv1::permanent_moved::Destination::Graveyard,
                            ));
                        }
                        if let (Some(cid), Some(ctrl)) = (card_id_t, owner) {
                            destroyed.push((tid, cid, ctrl));
                        }
                    }
                    for (tid, cid, ctrl) in destroyed {
                        self.fire_triggers(
                            GameEvent::Dies {
                                object_id: tid,
                                card_id: cid,
                                controller: ctrl,
                            },
                            events,
                        );
                    }
                }
                SpellEffectKind::DamageAll { amount, kind } => {
                    // CR 119: deal damage to each matching permanent. Marking damage mirrors
                    // DamageTarget; lethal-damage destruction is left to state-based actions
                    // (CR 704.5g), which run immediately after this spell resolves.
                    let affected = battlefield_objects_matching(&self.state, self.registry, &kind);
                    for tid in &affected {
                        let tgt = object_display_name(&self.state, self.registry, *tid);
                        if let Some(o) = self.state.objects.get_mut(tid) {
                            o.damage += amount;
                        }
                        events.push(ev_log(format!(
                            "{spell_label} deals {amount} damage to {tgt}"
                        )));
                    }
                }
                SpellEffectKind::CreateTokens {
                    token,
                    count,
                    controller: who,
                } => {
                    self.create_tokens(&token, count, who, controller, &spell_label, events);
                }
                // CR 605.3b: a mana ability never uses the stack, so a ProduceMana effect is
                // resolved immediately in `resolve_mana_ability` and can never reach this generic
                // stack-resolution path. Defensive no-op (registry validation also forbids it on
                // spells); producing mana here would be off-stack-timing and is intentionally not done.
                SpellEffectKind::ProduceMana { .. } => {}
                SpellEffectKind::None => {}
            }
        }
        events.push(ev_log(format!("{spell_label} resolves.")));
        Ok(())
    }

    /// CR 111: mint `count` tokens of `token_id` for each recipient and put them onto the
    /// battlefield. Each minted token is a fresh [`GameObject`] whose characteristics come from
    /// the token's [`CardDefinition`] (via the registry's token namespace), so combat, P/T, and
    /// keyword queries treat it exactly like any other permanent. Entering tokens fire ETB
    /// triggers (CR 603.6) through the same hook as a resolved creature spell, so Soul Warden et al.
    /// see them. A [`TokenCreated`](rv1::TokenCreated) event carries the self-describing identity
    /// the relay needs (tokens have no deck card / Oracle entry).
    fn create_tokens(
        &mut self,
        token_id: &str,
        count: u32,
        who: TokenController,
        spell_controller: PlayerId,
        spell_label: &str,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let registry = self.registry;
        let Some(def) = registry.get(token_id) else {
            // Registry load validates every CreateTokens reference, so this is unreachable;
            // fail safe by doing nothing rather than panicking (server-authoritative).
            events.push(ev_log(format!(
                "{spell_label} could not create unknown token '{token_id}'."
            )));
            return;
        };
        let name = def.name.clone();
        let is_creature = def.is_creature;
        let power = def.power;
        let toughness = def.toughness;
        let types = def.types.clone();
        let keywords: Vec<String> = def
            .keywords
            .iter()
            .map(|k| k.as_str().to_string())
            .collect();
        let color = color_string(&def.colors());
        let pt = if is_creature {
            format!("{}/{}", power.unwrap_or(0), toughness.unwrap_or(0))
        } else {
            String::new()
        };

        let recipients: Vec<PlayerId> = match who {
            TokenController::Controller => vec![spell_controller],
            // CR 111.3: each token's owner/controller is the player it is created under.
            TokenController::EachPlayer => self
                .state
                .players
                .iter()
                .filter(|p| !p.has_lost)
                .map(|p| p.id)
                .collect(),
        };

        for pid in recipients {
            let Some(pidx) = self.state.player_idx(pid) else {
                continue;
            };
            for _ in 0..count {
                let oid = self.state.next_object_id;
                self.state.next_object_id += 1;
                self.state.objects.insert(
                    oid,
                    GameObject {
                        id: oid,
                        owner: pid,
                        card_id: token_id.to_string(),
                        zone: Zone::Battlefield,
                        tapped: false,
                        summoning_sick: is_creature,
                        power,
                        toughness,
                        damage: 0,
                        deathtouch_damage: false,
                        counters: BTreeMap::new(),
                    },
                );
                self.state.players[pidx].battlefield.push(oid);
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::TokenCreated(rv1::TokenCreated {
                        object_id: oid,
                        controller_player_id: pid,
                        card_id: token_id.to_string(),
                        identity: Some(rv1::TokenIdentity {
                            name: name.clone(),
                            pt: pt.clone(),
                            color: color.clone(),
                            types: types.clone(),
                            is_creature,
                            keywords: keywords.clone(),
                        }),
                    })),
                });
                // CR 603.6: a token entering the battlefield triggers ETB watchers.
                self.fire_triggers(GameEvent::EntersBattlefield { object_id: oid }, events);
            }
            let noun = if count == 1 { "token" } else { "tokens" };
            events.push(ev_log(format!(
                "P{pid} creates {count} {name} {noun} ({spell_label})."
            )));
        }
    }

    fn cast_spell(
        &mut self,
        player: PlayerId,
        hand_idx: usize,
        targets: &[rv1::TargetRef],
        x_value: u32,
        flex_payments: &[rv1::FlexPipPayment],
        face_index: usize,
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
        // CR 709/712/715: cast the chosen face of a multi-face card (split half / MDFC face /
        // adventure half). Single-face cards have only face 0. Capture the face's characteristics
        // into owned locals so the (immutable) registry borrow can be released before mutating state.
        let face = def
            .face(face_index)
            .ok_or(EngineError::Illegal("bad face index"))?;
        if face.is_land {
            return Err(EngineError::Illegal("use play land"));
        }
        let face_is_sorcery = face.is_sorcery;
        let face_instant_speed = castable_at_instant_speed(&face);
        let face_mana = face.mana_cost.clone();
        let face_name = face.name.to_string();
        let face_effects: Vec<SpellEffectKind> = face.spell_effect.to_vec();
        let sorcery_ok = sorcery_speed_available(&self.state, player);
        let instant_ok = instant_timing_step_allowed(self.state.turn_step);
        // CR 601.3a: sorceries are sorcery-speed only; instants and flash cards (CR 702.8b)
        // use instant timing; every other permanent is sorcery-speed only.
        if face_is_sorcery {
            if !sorcery_ok {
                return Err(EngineError::Illegal("sorcery speed only"));
            }
        } else if face_instant_speed {
            if !instant_ok {
                return Err(EngineError::Illegal("instant timing"));
            }
        } else if !sorcery_ok {
            return Err(EngineError::Illegal("sorcery speed only"));
        }
        // CR 508.1 / 508.2: attackers are declared before any player gets priority in the
        // declare-attackers step. CR 509.1 / 509.2: same for blockers in declare blockers.
        if priority_locked_for_combat_declaration(&self.state) {
            return Err(EngineError::Illegal(
                "cannot cast until attack or block declaration is complete",
            ));
        }
        if !self.state.pending_triggers.is_empty() {
            return Err(EngineError::Illegal(
                "must choose trigger target before casting",
            ));
        }
        validate_spell_targets(&self.state, self.registry, player, &face_effects, targets)?;
        // CR 107.3: a chosen X is required iff the cost has an {X} pip; reject a stray x_value on a
        // non-X spell (strictness) and a missing one on an X spell. X=0 is a legal choice.
        let has_x = face_mana.has_x();
        if x_value != 0 && !has_x {
            return Err(EngineError::Illegal("x_value given but cost has no {X}"));
        }
        let chosen_x = if has_x { x_value } else { 0 };
        pay_mana(&mut self.state, idx, &face_mana, chosen_x, flex_payments)?;

        self.state.players[idx].hand.retain(|&x| x != oid);
        let trefs: Vec<ObjectId> = targets.iter().map(|t| t.object_id).collect();
        let tgt_line = format_spell_targets_log(&self.state, self.registry, &trefs);

        self.state.stack.push(StackItem {
            id: oid,
            controller: player,
            card_id: card_id.clone(),
            targets: trefs,
            ability_text: None,
            source_permanent_id: None,
            ability_index: None,
            is_triggered: false,
            is_copy: false,
            chosen_x,
            face_index,
        });
        if let Some(o) = self.state.objects.get_mut(&oid) {
            o.zone = Zone::Stack;
        }

        self.state.passes_since_stack_change = 0;
        // MTG priority: after casting a spell, the caster gets priority first.
        self.state.priority_idx = idx;

        // The cast face's own name (e.g. "Fire" / "Ice"), so logs and the stack card show the
        // half that was cast, not the whole-card `//` name.
        let cast_card_id = card_id.clone();
        let mut batch = RuledEventBatch::default();
        let x_line = if has_x {
            format!(" (X={chosen_x})")
        } else {
            String::new()
        };
        batch.events.push(ev_log(format!(
            "P{} casts {}{}{}",
            player, face_name, x_line, tgt_line
        )));
        batch.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                object_id: oid,
                description: face_name,
                targets: targets.to_vec(),
                ability_annotation: String::new(),
                card_id: cast_card_id.clone(),
                is_copy: false,
            })),
        });
        self.fire_triggers(
            GameEvent::SpellCast {
                caster: player,
                card_id: cast_card_id,
            },
            &mut batch.events,
        );
        batch.events.push(ev_priority_changed(self));
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    fn activate_ability(
        &mut self,
        player: PlayerId,
        permanent_id: u32,
        ability_index: usize,
        targets: &[rv1::TargetRef],
        flex_payments: &[rv1::FlexPipPayment],
        mana_option_index: u32,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        if self.state.turn_step == TurnStep::Cleanup {
            return Err(EngineError::Illegal("no abilities during cleanup"));
        }
        if priority_locked_for_combat_declaration(&self.state) {
            return Err(EngineError::Illegal(
                "cannot activate until attack or block declaration is complete",
            ));
        }
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;

        // Validate the permanent exists on the battlefield and is controlled by this player.
        let card_id = self
            .state
            .objects
            .get(&permanent_id)
            .filter(|o| o.zone == Zone::Battlefield)
            .map(|o| o.card_id.clone())
            .ok_or(EngineError::Illegal("permanent not on battlefield"))?;
        if !self.state.players[idx].battlefield.contains(&permanent_id) {
            return Err(EngineError::Illegal("not your permanent"));
        }

        let def = self
            .registry
            .get(&card_id)
            .ok_or_else(|| EngineError::MissingCard(card_id.clone()))?;

        let ability = def
            .activated_abilities
            .get(ability_index)
            .ok_or(EngineError::Illegal("no such activated ability"))?
            .clone();

        // CR 605.1a: classify mana abilities by their effect (ProduceMana). They don't use the
        // stack (CR 605.3a) and resolve immediately, so they take a wholly separate path below —
        // before target validation (a mana ability is untargeted) and before the stack push.
        if matches!(ability.effect, SpellEffectKind::ProduceMana { .. }) {
            return self.resolve_mana_ability(
                player,
                idx,
                permanent_id,
                &card_id,
                &ability,
                mana_option_index,
                targets,
                flex_payments,
            );
        }

        // CR 602.2: validate targets BEFORE paying cost.
        validate_effect_targets(&self.state, self.registry, player, &ability.effect, targets)?;

        // Pay the cost. Track sacrifice separately so we can emit a PermanentMoved event below.
        let sacrifice_ev = self.pay_ability_cost(
            player,
            idx,
            permanent_id,
            &card_id,
            &ability.cost,
            flex_payments,
        )?;

        let trefs: Vec<ObjectId> = targets.iter().map(|t| t.object_id).collect();
        let ability_text = ability.text.clone();
        let card_name = self
            .registry
            .get(&card_id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| card_id.clone());

        // Allocate a virtual ObjectId for the ability on the stack (not added to state.objects).
        let virtual_id = self.state.next_object_id;
        self.state.next_object_id += 1;

        self.state.stack.push(StackItem {
            id: virtual_id,
            controller: player,
            card_id: card_id.clone(),
            targets: trefs.clone(),
            ability_text: Some(ability_text.clone()),
            source_permanent_id: Some(permanent_id),
            ability_index: Some(ability_index),
            is_triggered: false,
            is_copy: false,
            // Abilities don't have an {X} cast value (X in activated-ability costs is deferred).
            chosen_x: 0,
            // Abilities are not faces; the front face (0) is the conventional default.
            face_index: 0,
        });
        self.state.passes_since_stack_change = 0;
        self.state.priority_idx = idx;

        let tgt_line = format_spell_targets_log(&self.state, self.registry, &trefs);
        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!(
            "P{player} activates {card_name}: {ability_text}{tgt_line}"
        )));
        if let Some(ev) = sacrifice_ev {
            batch.events.push(ev);
        }
        batch.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                object_id: virtual_id,
                description: card_name,
                targets: targets.to_vec(),
                ability_annotation: ability_text,
                card_id: String::new(),
                is_copy: false,
            })),
        });
        batch.events.push(ev_priority_changed(self));
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    /// Pay an activated ability's cost (CR 602.2) atomically. Shared by the stack path
    /// ([`Self::activate_ability`]) and the no-stack mana-ability path
    /// ([`Self::resolve_mana_ability`]). Mana is checked before the source is tapped so a failed
    /// payment leaves the permanent untapped (CR 601.2h). Returns the sacrifice `PermanentMoved`
    /// event when the cost was a sacrifice, so the caller can surface it.
    fn pay_ability_cost(
        &mut self,
        player: PlayerId,
        idx: usize,
        permanent_id: ObjectId,
        card_id: &str,
        cost: &AbilityCost,
        flex_payments: &[rv1::FlexPipPayment],
    ) -> Result<Option<rv1::RuledEvent>, EngineError> {
        match cost {
            AbilityCost::Tap => {
                self.tap_for_cost(permanent_id, card_id)?;
                Ok(None)
            }
            AbilityCost::Mana(cost) => {
                // X in activated-ability costs is deferred; abilities always pass X=0.
                pay_mana(&mut self.state, idx, cost, 0, flex_payments)?;
                Ok(None)
            }
            AbilityCost::TapAndMana(cost) => {
                // Check mana sufficiency BEFORE tapping so a failed payment leaves the
                // permanent untapped (costs are paid atomically in MTG CR 601.2h).
                pay_mana(&mut self.state, idx, cost, 0, flex_payments)?;
                self.tap_for_cost(permanent_id, card_id)?;
                Ok(None)
            }
            AbilityCost::Sacrifice => {
                let owner = self
                    .state
                    .objects
                    .get(&permanent_id)
                    .map(|o| o.owner)
                    .unwrap_or(player);
                sacrifice_permanent(&mut self.state, permanent_id)?;
                Ok(Some(permanent_moved_event(
                    &self.state,
                    permanent_id,
                    owner,
                    rv1::permanent_moved::Destination::Graveyard,
                )))
            }
        }
    }

    /// Tap `permanent_id` as a {T} cost component, rejecting an already-tapped or
    /// summoning-sick source (CR 302.6 / 702.10 — Haste exempts the latter).
    fn tap_for_cost(&mut self, permanent_id: ObjectId, card_id: &str) -> Result<(), EngineError> {
        let has_haste = self
            .registry
            .get(card_id)
            .map(|d| d.keywords.contains(&tricerules_cards::Keyword::Haste))
            .unwrap_or(false);
        let o = self
            .state
            .objects
            .get_mut(&permanent_id)
            .ok_or(EngineError::Illegal("permanent missing"))?;
        if o.tapped {
            return Err(EngineError::Illegal("permanent is already tapped"));
        }
        if o.summoning_sick && !has_haste {
            return Err(EngineError::Illegal(
                "cannot use tap ability due to summoning sickness",
            ));
        }
        o.tapped = true;
        Ok(())
    }

    /// Resolve a mana ability (CR 605) immediately, with no stack and no priority change
    /// (CR 605.3a–b). Pays the cost, adds the chosen `ProduceMana` option to the player's pool,
    /// and emits the pool update + a log line. The tap (when the cost is `{T}`) reaches clients
    /// via the batch's normal `ev_zone_view_sync` (`battlefield_tapped`); no opponent gets a
    /// window to respond, matching the rule that mana abilities resolve as they are activated.
    #[allow(clippy::too_many_arguments)]
    fn resolve_mana_ability(
        &mut self,
        player: PlayerId,
        idx: usize,
        permanent_id: ObjectId,
        card_id: &str,
        ability: &tricerules_cards::ActivatedAbilityDef,
        mana_option_index: u32,
        targets: &[rv1::TargetRef],
        flex_payments: &[rv1::FlexPipPayment],
    ) -> Result<RuledEventBatch, EngineError> {
        // A mana ability is untargeted (CR 605.1a).
        if !targets.is_empty() {
            return Err(EngineError::Illegal("mana ability takes no targets"));
        }
        let SpellEffectKind::ProduceMana { options } = &ability.effect else {
            return Err(EngineError::Illegal("not a mana ability"));
        };
        let amount = options
            .get(mana_option_index as usize)
            .ok_or(EngineError::Illegal("invalid mana option"))?;

        // Pay the cost first (CR 602.2). On failure nothing has changed yet (atomic).
        let sacrifice_ev = self.pay_ability_cost(
            player,
            idx,
            permanent_id,
            card_id,
            &ability.cost,
            flex_payments,
        )?;

        // Add the produced mana to the player's pool (CR 106.4).
        let pool = &mut self.state.players[idx].mana_pool;
        pool.white += amount.w;
        pool.blue += amount.u;
        pool.black += amount.b;
        pool.red += amount.r;
        pool.green += amount.g;
        pool.colorless += amount.c;

        let card_name = self
            .registry
            .get(card_id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| card_id.to_string());
        let ability_text = ability.text.clone();

        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!(
            "P{player} activates {card_name}: {ability_text}"
        )));
        if let Some(ev) = sacrifice_ev {
            batch.events.push(ev);
        }
        // The pool update reaches clients via the per-batch ManaPoolUpdated emit in apply_command.
        // No stack push, no priority change, no passes_since_stack_change reset (CR 605.3a–b).
        Ok(batch)
    }

    fn choose_trigger_target(
        &mut self,
        player: PlayerId,
        target_object_id: u32,
    ) -> Result<RuledEventBatch, EngineError> {
        let pending = self
            .state
            .pending_triggers
            .pop_front()
            .ok_or(EngineError::Illegal("no pending trigger awaiting target"))?;

        if pending.controller != player {
            // Put it back at the front
            self.state.pending_triggers.push_front(pending);
            return Err(EngineError::Illegal("not your trigger to target"));
        }

        let def = self
            .registry
            .get(&pending.card_id)
            .ok_or_else(|| EngineError::MissingCard(pending.card_id.clone()))?;

        let effect = def
            .triggered_abilities
            .get(pending.ability_index)
            .map(|a| &a.effect);

        // Validate the chosen target against the effect's target spec.
        let target_ref = &[rv1::TargetRef {
            object_id: target_object_id,
        }];
        if let Some(kind) = effect {
            validate_effect_targets(&self.state, self.registry, player, kind, target_ref)?;
        }

        let virtual_id = self.state.next_object_id;
        self.state.next_object_id += 1;

        let ability_text = pending.ability_text.clone();
        let card_name = def.name.clone();
        let card_id = pending.card_id.clone();
        let source_id = pending.source_permanent_id;
        let ability_index = pending.ability_index;
        let controller = pending.controller;

        let trefs = vec![target_object_id];
        let tgt_line = format_spell_targets_log(&self.state, self.registry, &trefs);

        self.state.stack.push(StackItem {
            id: virtual_id,
            controller,
            card_id: card_id.clone(),
            targets: trefs,
            ability_text: Some(ability_text.clone()),
            source_permanent_id: Some(source_id),
            ability_index: Some(ability_index),
            is_triggered: true,
            is_copy: false,
            chosen_x: 0,
            face_index: 0,
        });
        self.state.passes_since_stack_change = 0;

        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!(
            "P{controller} {card_name} trigger targets{tgt_line}"
        )));
        batch.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                object_id: virtual_id,
                description: card_name,
                targets: vec![rv1::TargetRef {
                    object_id: target_object_id,
                }],
                ability_annotation: ability_text,
                card_id: String::new(),
                is_copy: false,
            })),
        });

        // If another pending trigger is waiting for target selection, prompt for it now.
        if let Some(next) = self.state.pending_triggers.front() {
            let next_name = self
                .registry
                .get(&next.card_id)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| next.card_id.clone());
            batch.events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::TriggerNeedsTarget(
                    rv1::TriggerNeedsTarget {
                        source_permanent_id: next.source_permanent_id,
                        ability_index: next.ability_index as u32,
                        ability_text: next.ability_text.clone(),
                        controller_player_id: next.controller,
                    },
                )),
            });
            batch.events.push(ev_log(format!(
                "Triggered: {next_name} — choose a target for: {}",
                next.ability_text
            )));
        } else {
            batch.events.push(ev_priority_changed(self));
        }
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    /// Begin a tier-3 custom resolution (CR 608). Runs the [`CardEffect::begin`] step and either
    /// completes (resolution falls through normally) or parks the spell awaiting a player choice.
    fn begin_custom_resolution(
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

    /// Apply a deciding player's answer to the outstanding [`PendingResolution`] (CR 608) and
    /// resume the [`CardEffect`], looping conceptually until it completes or asks for more input.
    fn submit_resolution_choice(
        &mut self,
        player: PlayerId,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let pending = self
            .state
            .pending_resolution
            .take()
            .ok_or(EngineError::Illegal("no resolution awaiting a choice"))?;
        // Restore the parked state on any rejection so an illegal submission never mutates state.
        if pending.deciding_player != player {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("not your resolution choice"));
        }
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

        // When the resolution finished (did not re-park), hand priority back to the active
        // player so the game resumes normally (the apply_command tail runs SBAs + zone view).
        if self.state.pending_resolution.is_none() {
            if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
                self.state.priority_idx = i;
            }
            ev.push(ev_priority_changed(self));
        }
        Ok(finish_with_events(self, ev))
    }

    /// Shared tail of `begin`/`resume`: either let resolution complete, or park the in-flight
    /// spell as a [`PendingResolution`] and emit the `ResolutionChoiceRequired` request. `step_no`
    /// is the step that produced this outcome; the next resume runs as `step_no + 1`.
    fn park_or_finish(
        &mut self,
        item: StackItem,
        custom_key: String,
        step_no: u32,
        scratch: Vec<ObjectId>,
        step: ResolutionStep,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let interrupt = match step {
            ResolutionStep::Done => return,
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
        // Oracle display names for the deciding player's client (private hand cards are redacted
        // per-player by Servatrice). `CardDefinition.name` is the Oracle name — same source the
        // engine already uses for `StackPushed.description`.
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
                    choice_kind: interrupt.choice_kind.as_proto(),
                    candidate_object_ids: interrupt.candidates.clone(),
                    candidate_card_ids,
                    candidate_names,
                    min: interrupt.min,
                    max: interrupt.max,
                    ordered: interrupt.ordered,
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
            prompt: interrupt.prompt,
            choice_kind: interrupt.choice_kind.as_proto(),
        });
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
        let land_name = def.name.clone();
        batch
            .events
            .push(ev_log(format!("P{} played {}", player, land_name)));
        self.fire_triggers(
            GameEvent::EntersBattlefield { object_id: oid },
            &mut batch.events,
        );
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    fn apply_sbas(&mut self, out: &mut Vec<rv1::RuledEvent>) -> Result<(), EngineError> {
        // CR 122.3: if a permanent has both +1/+1 and -1/-1 counters, N of each are removed as a
        // state-based action, where N is the smaller count. Done before the toughness/lethal-damage
        // death check below so a net-zero creature is evaluated on its true (post-annihilation) P/T.
        for o in self.state.objects.values_mut() {
            if o.zone != Zone::Battlefield {
                continue;
            }
            let plus = o.counter_count(CounterKind::PlusOnePlusOne);
            let minus = o.counter_count(CounterKind::MinusOneMinusOne);
            let pairs = plus.min(minus);
            if pairs > 0 {
                o.set_counter(CounterKind::PlusOnePlusOne, plus - pairs);
                o.set_counter(CounterKind::MinusOneMinusOne, minus - pairs);
            }
        }

        // Collect battlefield creature IDs first to avoid borrow conflict when calling
        // effective_toughness (which borrows self.state while objects iter already borrows it).
        let candidate_ids: Vec<ObjectId> = self
            .state
            .objects
            .iter()
            .filter(|(_, o)| o.zone == Zone::Battlefield && o.toughness.is_some())
            .map(|(id, _)| *id)
            .collect();

        let mut to_destroy = Vec::new();
        for id in candidate_ids {
            let Some(eff_t) = self.effective_toughness(id) else {
                continue;
            };
            let Some(o) = self.state.objects.get(&id) else {
                continue;
            };
            let indestructible = o.has_keyword(self.registry, Keyword::Indestructible);
            // CR 704.5f: toughness 0 — still dies even with indestructible (oracle text).
            if eff_t == 0 {
                to_destroy.push(id);
            // CR 704.5g / 704.5h: lethal damage or deathtouch — blocked by indestructible.
            } else if !indestructible && (o.damage >= eff_t || o.deathtouch_damage) {
                to_destroy.push(id);
            }
        }
        for id in to_destroy {
            let owner = self.state.objects.get(&id).map(|o| o.owner);
            let card_id_for_trigger = self.state.objects.get(&id).map(|o| o.card_id.clone());
            if destroy_permanent(&mut self.state, id).is_ok() {
                if let Some(owner_id) = owner {
                    out.push(permanent_moved_event(
                        &self.state,
                        id,
                        owner_id,
                        rv1::permanent_moved::Destination::Graveyard,
                    ));
                }
                if let (Some(cid), Some(ctrl)) = (card_id_for_trigger, owner) {
                    self.fire_triggers(
                        GameEvent::Dies {
                            object_id: id,
                            card_id: cid,
                            controller: ctrl,
                        },
                        out,
                    );
                }
            }
        }

        // CR 111.7/111.8: a token that has left the battlefield ceases to exist as a state-based
        // action. The token already moved to its destination zone above (so any dies/leaves
        // triggers saw it there and a PermanentMoved was emitted for the relay), so here we just
        // delete the object entirely — it must not linger in a graveyard/hand/exile, nor return.
        let vanished: Vec<ObjectId> = self
            .state
            .objects
            .iter()
            .filter(|(_, o)| o.zone != Zone::Battlefield && o.is_token(self.registry))
            .map(|(id, _)| *id)
            .collect();
        for id in vanished {
            if let Some(o) = self.state.objects.remove(&id) {
                if let Some(pidx) = self.state.player_idx(o.owner) {
                    let p = &mut self.state.players[pidx];
                    p.hand.retain(|&x| x != id);
                    p.battlefield.retain(|&x| x != id);
                    p.graveyard.retain(|&x| x != id);
                    p.exile.retain(|&x| x != id);
                    p.library.retain(|&x| x != id);
                }
            }
        }

        // CR 704.5j: the legend rule is a state-based action, so it must be checked on every SBA
        // pass — not only at the combat-damage steps and the turn roll where it was historically
        // wired up. Otherwise a second same-named legend that enters mid-main-phase (e.g. a second
        // legendary creature spell resolving) would illegally coexist until the next combat or
        // cleanup. `apply_legend_sbas` is idempotent: once a duplicate has moved to the graveyard
        // its `zone != Battlefield` excludes it, so the pre-existing explicit call sites that still
        // run alongside this one find nothing and emit no duplicate events.
        let legend_events = self.apply_legend_sbas()?;
        out.extend(legend_events);
        Ok(())
    }

    fn apply_legend_sbas(&mut self) -> Result<Vec<rv1::RuledEvent>, EngineError> {
        // CR 704.5j: the legend rule applies per controller — only same-named legends controlled
        // by the *same* player conflict, so two players may each control a copy. Keyed by
        // (controller, name); `BTreeMap` + sorted ids keep the kept-permanent choice deterministic
        // for replay (we keep the lowest object id; controller-chosen keep is not modeled yet).
        let mut by_owner_name: BTreeMap<(PlayerId, String), Vec<ObjectId>> = BTreeMap::new();
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
            by_owner_name.entry((o.owner, n)).or_default().push(id);
        }
        for ids in by_owner_name.values_mut() {
            if ids.len() < 2 {
                continue;
            }
            ids.sort_unstable();
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
        // Catalog first: Servatrice resolves the zone-view card ids below through it.
        batch.events.push(self.ev_card_catalog());
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
        // Per-command responses: the version handshake fields are SessionStart-only, so
        // leave them at their defaults here.
        match RuledCommand::decode(bytes) {
            Ok(cmd) => match self.apply_command(player, &cmd) {
                Ok(batch) => IpcResponse {
                    ok: true,
                    error: String::new(),
                    batch: Some(batch),
                    missing_card_names: vec![],
                    ..Default::default()
                },
                Err(EngineError::GameOver(w)) => IpcResponse {
                    ok: true,
                    error: String::new(),
                    batch: Some(self.game_over_batch_winner(w)),
                    missing_card_names: vec![],
                    ..Default::default()
                },
                Err(e) => IpcResponse {
                    ok: false,
                    error: e.to_string(),
                    batch: None,
                    missing_card_names: vec![],
                    ..Default::default()
                },
            },
            Err(e) => IpcResponse {
                ok: false,
                error: format!("decode: {e}"),
                batch: None,
                missing_card_names: vec![],
                ..Default::default()
            },
        }
    }

    /// Engine-owned card identity for the session: the union of all deck card ids mapped
    /// to Oracle names plus the mechanical info Servatrice needs without querying Oracle.
    /// Server-only — Servatrice strips it from client broadcasts (it enumerates decks).
    fn ev_card_catalog(&self) -> RuledEvent {
        // BTreeSet: dedupe + deterministic entry order for replays.
        let ids: BTreeSet<&str> = self
            .state
            .objects
            .values()
            .map(|o| o.card_id.as_str())
            .collect();
        let entries = ids
            .into_iter()
            .filter_map(|id| self.registry.get(id))
            .map(|def| rv1::card_catalog::Entry {
                card_id: def.id.clone(),
                name: def.name.clone(),
                // Multi-face cards carry no flat types; describe by the primary (front) face.
                types: def.primary_face().types.to_vec(),
                is_permanent: def.primary_face().is_permanent(),
                // CR 709/712/715: per-face names so the relay can display a cast/active half;
                // empty for single-face cards.
                face_names: if def.is_multiface() {
                    def.faces.iter().map(|f| f.name.clone()).collect()
                } else {
                    Vec::new()
                },
            })
            .collect();
        RuledEvent {
            ev: Some(rv1::ruled_event::Ev::CardCatalog(rv1::CardCatalog {
                entries,
            })),
        }
    }

    /// Deck + hand for Cockatrice server to line up with tricerules state.
    /// Build a `ManaPoolUpdated` event (CR 106) carrying player `idx`'s current absolute pool.
    fn ev_mana_pool_updated(&self, idx: usize) -> RuledEvent {
        let p = &self.state.players[idx];
        let pool = &p.mana_pool;
        RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ManaPoolUpdated(
                rv1::ManaPoolUpdated {
                    player_id: p.id,
                    w: pool.white,
                    u: pool.blue,
                    b: pool.black,
                    r: pool.red,
                    g: pool.green,
                    c: pool.colorless,
                },
            )),
        }
    }

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
                        if self
                            .state
                            .objects
                            .get(&oid)
                            .is_some_and(|o| o.is_creature(self.registry))
                        {
                            self.effective_power(oid).unwrap_or(0)
                        } else {
                            0
                        }
                    })
                    .collect(),
                battlefield_toughness: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        if self
                            .state
                            .objects
                            .get(&oid)
                            .is_some_and(|o| o.is_creature(self.registry))
                        {
                            self.effective_toughness(oid).unwrap_or(0)
                        } else {
                            0
                        }
                    })
                    .collect(),
                battlefield_damage: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .filter(|o| o.is_creature(self.registry))
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
                            .map(|o| o.is_creature(self.registry))
                            .unwrap_or(false)
                    })
                    .collect(),
                // CR 702.10: clients use this to suppress the summoning-sick indicator
                // and allow attacker selection for creatures that entered this turn.
                battlefield_haste: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| o.has_keyword(self.registry, tricerules_cards::Keyword::Haste))
                            .unwrap_or(false)
                    })
                    .collect(),
                // CR 702.19: clients use this to enable trample damage assignment UI.
                battlefield_trample: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| {
                                o.has_keyword(self.registry, tricerules_cards::Keyword::Trample)
                            })
                            .unwrap_or(false)
                    })
                    .collect(),
                // CR 702.7: informational flag for the client UI (independent of pending state).
                battlefield_first_strike: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| {
                                o.has_keyword(self.registry, tricerules_cards::Keyword::FirstStrike)
                            })
                            .unwrap_or(false)
                    })
                    .collect(),
                // CR 702.4: informational flag for the client UI.
                battlefield_double_strike: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .map(|o| {
                                o.has_keyword(
                                    self.registry,
                                    tricerules_cards::Keyword::DoubleStrike,
                                )
                            })
                            .unwrap_or(false)
                    })
                    .collect(),
                // CR 510.4: true while combat is set up with at least one attacker or blocker
                // having FirstStrike/DoubleStrike and the first-strike step has not yet resolved.
                first_strike_step_pending: self
                    .state
                    .combat
                    .as_ref()
                    .map(|c| {
                        !c.first_strike_damage_done
                            && combat_needs_first_strike_step(&self.state, self.registry, c)
                    })
                    .unwrap_or(false),
                // Pipe-delimited activated ability texts per battlefield permanent (empty if none).
                battlefield_activated_ability_texts: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .and_then(|o| self.registry.get(&o.card_id))
                            .map(|def| {
                                def.activated_abilities
                                    .iter()
                                    .map(|a| a.text.as_str())
                                    .collect::<Vec<_>>()
                                    .join("|")
                            })
                            .unwrap_or_default()
                    })
                    .collect(),
                // Parallel to `battlefield_activated_ability_texts`: pipe-delimited mana cost
                // strings extracted from AbilityCost in canonical Scryfall brace form. Tap/Sacrifice
                // → "", Mana/TapAndMana → "{4}"/"{R}"/etc. The client parses both braces and the
                // legacy compact form (see PlayerActions::parseSimpleManaCost).
                battlefield_activated_ability_mana_costs: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .and_then(|o| self.registry.get(&o.card_id))
                            .map(|def| {
                                def.activated_abilities
                                    .iter()
                                    .map(|a| match &a.cost {
                                        AbilityCost::Mana(c) | AbilityCost::TapAndMana(c) => {
                                            c.to_string()
                                        }
                                        _ => String::new(),
                                    })
                                    .collect::<Vec<_>>()
                                    .join("|")
                            })
                            .unwrap_or_default()
                    })
                    .collect(),
                // Parallel to `battlefield_activated_ability_texts`: mana-ability production (CR 605).
                battlefield_activated_ability_mana_produced: p
                    .battlefield
                    .iter()
                    .map(|&oid| {
                        self.state
                            .objects
                            .get(&oid)
                            .and_then(|o| self.registry.get(&o.card_id))
                            .map(|def| {
                                def.activated_abilities
                                    .iter()
                                    .map(|a| match &a.effect {
                                        SpellEffectKind::ProduceMana { options } => options
                                            .iter()
                                            .map(mana_amount_symbols)
                                            .collect::<Vec<_>>()
                                            .join("/"),
                                        _ => String::new(),
                                    })
                                    .collect::<Vec<_>>()
                                    .join("|")
                            })
                            .unwrap_or_default()
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

    // -----------------------------------------------------------------------
    // Trigger detection — unified event-driven pass (CR 603.2)
    // -----------------------------------------------------------------------

    /// Emit a game event and enqueue all matching triggered abilities (CR 603.3b APNAP order).
    /// Replaces the old per-condition `fire_*` helpers.
    fn fire_triggers(&mut self, event: GameEvent, events: &mut Vec<rv1::RuledEvent>) {
        let triggers = self.collect_triggers(&event);
        for (source_id, card_id, controller, ability_index, ability_text) in triggers {
            self.push_trigger(
                source_id,
                &card_id,
                controller,
                ability_index,
                ability_text,
                events,
            );
        }
    }

    /// Collect `(source_id, card_id, controller, ability_index, ability_text)` for every triggered
    /// ability whose condition matches `event`. Returns owned data so the caller can mutate `self`.
    /// Results are ordered APNAP: active player's triggers first (CR 603.3b).
    fn collect_triggers(
        &self,
        event: &GameEvent,
    ) -> Vec<(ObjectId, String, PlayerId, usize, String)> {
        let ap = self.state.active_player_id();
        match event {
            GameEvent::EntersBattlefield { object_id } => {
                let Some(obj) = self.state.objects.get(object_id) else {
                    return vec![];
                };
                let entering_id = *object_id;
                let entering_card_id = obj.card_id.clone();
                let entering_controller = obj.owner;
                // Cloned once for type matching by ETB-watchers below (cheap, once per ETB).
                let entering_def = self.registry.get(&entering_card_id).cloned();

                let mut out = Vec::new();
                // (1) The entering permanent's own ETB triggers (Elvish Visionary, Flametongue …).
                out.extend(self.matching_triggered_abilities(
                    &entering_card_id,
                    entering_id,
                    entering_controller,
                    |tc| *tc == TriggerCondition::WhenSelfEntersBattlefield,
                ));

                // (2) Other permanents watching for permanents entering — Soul Warden,
                // landfall, constellation. Deterministic APNAP order: active player first.
                let mut ordered: Vec<usize> = (0..self.state.players.len()).collect();
                ordered.sort_by_key(|&i| (self.state.players[i].id != ap) as u8);
                let mut sources: Vec<(ObjectId, String, PlayerId)> = Vec::new();
                for pi in ordered {
                    for &sid in &self.state.players[pi].battlefield {
                        if let Some(o) = self.state.objects.get(&sid) {
                            sources.push((sid, o.card_id.clone(), o.owner));
                        }
                    }
                }
                let entering_def_ref = entering_def.as_ref();
                for (src_id, src_card, src_ctrl) in sources {
                    out.extend(self.matching_triggered_abilities(
                        &src_card,
                        src_id,
                        src_ctrl,
                        |tc| {
                            let TriggerCondition::WheneverPermanentEntersBattlefield {
                                controller,
                                permanent_type,
                                exclude_self,
                            } = tc
                            else {
                                return false;
                            };
                            // CR "another": the source's own entry doesn't trigger it.
                            if *exclude_self && src_id == entering_id {
                                return false;
                            }
                            let rel_ok = match controller {
                                tricerules_cards::CastTriggerPlayer::Controller => {
                                    entering_controller == src_ctrl
                                }
                                tricerules_cards::CastTriggerPlayer::Opponent => {
                                    entering_controller != src_ctrl
                                }
                                tricerules_cards::CastTriggerPlayer::AnyPlayer => true,
                            };
                            if !rel_ok {
                                return false;
                            }
                            match permanent_type {
                                Some(pt) => entering_def_ref
                                    .map(|d| d.is_permanent_type(*pt))
                                    .unwrap_or(false),
                                None => true,
                            }
                        },
                    ));
                }
                out
            }
            GameEvent::Dies {
                object_id,
                card_id,
                controller,
            } => self.matching_triggered_abilities(card_id, *object_id, *controller, |tc| {
                *tc == TriggerCondition::WhenSelfDies
            }),
            GameEvent::Attacks { attacker_ids } => {
                // APNAP: active player's attackers first, then NAP's
                let mut sorted = attacker_ids.clone();
                sorted.sort_by_key(|&oid| {
                    self.state
                        .objects
                        .get(&oid)
                        .map(|o| (o.owner != ap) as u8)
                        .unwrap_or(1)
                });
                sorted
                    .iter()
                    .flat_map(|&att| {
                        let Some(obj) = self.state.objects.get(&att) else {
                            return vec![];
                        };
                        let card_id = obj.card_id.clone();
                        let controller = obj.owner;
                        self.matching_triggered_abilities(&card_id, att, controller, |tc| {
                            *tc == TriggerCondition::WheneverSelfAttacks
                        })
                    })
                    .collect()
            }
            GameEvent::CombatDamageToPlayer {
                attacker_id,
                defender_id,
            } => {
                let Some(obj) = self.state.objects.get(attacker_id) else {
                    return vec![];
                };
                let card_id = obj.card_id.clone();
                let controller = obj.owner;
                let defender = *defender_id;
                self.matching_triggered_abilities(&card_id, *attacker_id, controller, |tc| match tc
                {
                    TriggerCondition::WheneverSelfDealsCombatDamageToPlayer => true,
                    TriggerCondition::WheneverSelfDealsDamageToOpponent => defender != controller,
                    _ => false,
                })
            }
            GameEvent::UpkeepBegin => {
                let ap_idx = self.state.player_idx(ap).unwrap_or(0);
                let bf: Vec<ObjectId> = self.state.players[ap_idx].battlefield.clone();
                bf.iter()
                    .flat_map(|&oid| {
                        let Some(obj) = self.state.objects.get(&oid) else {
                            return vec![];
                        };
                        let card_id = obj.card_id.clone();
                        let controller = obj.owner;
                        self.matching_triggered_abilities(&card_id, oid, controller, |tc| {
                            *tc == TriggerCondition::AtBeginningOfControllerUpkeep
                        })
                    })
                    .collect()
            }
            GameEvent::SpellCast {
                caster,
                card_id: cast_card_id,
            } => {
                let cast_def = self.registry.get(cast_card_id);
                let is_enchantment = cast_def.map(|d| d.is_enchantment).unwrap_or(false);
                let is_instant = cast_def.map(|d| d.is_instant).unwrap_or(false);
                let is_sorcery = cast_def.map(|d| d.is_sorcery).unwrap_or(false);
                let is_creature = cast_def.map(|d| d.is_creature).unwrap_or(false);
                let is_artifact = cast_def.map(|d| d.is_artifact).unwrap_or(false);

                // Deterministic APNAP order: active player's permanents first (CR 603.3b).
                // Iterating the raw `objects` HashMap here would be non-deterministic and break
                // replay reconstruction when two permanents (Young Pyromancer + Guttersnipe …)
                // both watch for spells being cast.
                let mut ordered: Vec<usize> = (0..self.state.players.len()).collect();
                ordered.sort_by_key(|&i| (self.state.players[i].id != ap) as u8);
                let mut sources: Vec<(ObjectId, String, PlayerId)> = Vec::new();
                for pi in ordered {
                    for &sid in &self.state.players[pi].battlefield {
                        if let Some(o) = self.state.objects.get(&sid) {
                            sources.push((sid, o.card_id.clone(), o.owner));
                        }
                    }
                }
                sources
                    .into_iter()
                    .flat_map(|(oid, card_id, source_controller)| {
                        self.matching_triggered_abilities(&card_id, oid, source_controller, |tc| {
                            let TriggerCondition::WheneverPlayerCastsSpell {
                                caster: caster_filter,
                                spell_type,
                            } = tc
                            else {
                                return false;
                            };
                            // Check caster matches.
                            let caster_ok = match caster_filter {
                                CastTriggerPlayer::Controller => *caster == source_controller,
                                CastTriggerPlayer::Opponent => *caster != source_controller,
                                CastTriggerPlayer::AnyPlayer => true,
                            };
                            if !caster_ok {
                                return false;
                            }
                            // Check spell type matches.
                            match spell_type {
                                None => true,
                                Some(SpellTypeFilter::Enchantment) => is_enchantment,
                                Some(SpellTypeFilter::Instant) => is_instant,
                                Some(SpellTypeFilter::Sorcery) => is_sorcery,
                                Some(SpellTypeFilter::InstantOrSorcery) => is_instant || is_sorcery,
                                Some(SpellTypeFilter::Creature) => is_creature,
                                Some(SpellTypeFilter::Artifact) => is_artifact,
                                Some(SpellTypeFilter::Noncreature) => !is_creature,
                            }
                        })
                    })
                    .collect()
            }
        }
    }

    /// Return all triggered abilities on `card_id` whose condition satisfies `filter`, as owned tuples.
    fn matching_triggered_abilities(
        &self,
        card_id: &str,
        source_id: ObjectId,
        controller: PlayerId,
        filter: impl Fn(&TriggerCondition) -> bool,
    ) -> Vec<(ObjectId, String, PlayerId, usize, String)> {
        let Some(def) = self.registry.get(card_id) else {
            return vec![];
        };
        def.triggered_abilities
            .iter()
            .enumerate()
            .filter(|(_, ta)| filter(&ta.trigger))
            .map(|(idx, ta)| {
                (
                    source_id,
                    card_id.to_string(),
                    controller,
                    idx,
                    ta.text.clone(),
                )
            })
            .collect()
    }

    /// Core helper: either push a non-targeted trigger to the stack immediately, or set
    /// `pending_trigger` and emit `TriggerNeedsTarget` for targeted ones.
    fn push_trigger(
        &mut self,
        source_id: ObjectId,
        card_id: &str,
        controller: PlayerId,
        ability_index: usize,
        ability_text: String,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let def = match self.registry.get(card_id) {
            Some(d) => d.clone(),
            None => return,
        };
        let needs_target = def
            .triggered_abilities
            .get(ability_index)
            .map(|ta| spell_effect_kind_needs_target(&ta.effect))
            .unwrap_or(false);

        let card_name = def.name.clone();
        let virtual_id = self.state.next_object_id;
        self.state.next_object_id += 1;

        if needs_target {
            // Enqueue for target selection; only emit TriggerNeedsTarget for the first in queue
            // so the client prompts one at a time (CR 603.3d).
            let was_empty = self.state.pending_triggers.is_empty();
            self.state.pending_triggers.push_back(PendingTrigger {
                source_permanent_id: source_id,
                ability_index,
                ability_text: ability_text.clone(),
                card_id: card_id.to_string(),
                controller,
            });
            if was_empty {
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::TriggerNeedsTarget(
                        rv1::TriggerNeedsTarget {
                            source_permanent_id: source_id,
                            ability_index: ability_index as u32,
                            ability_text: ability_text.clone(),
                            controller_player_id: controller,
                        },
                    )),
                });
                events.push(ev_log(format!(
                    "Triggered: {card_name} — choose a target for: {ability_text}"
                )));
            }
        } else {
            self.state.stack.push(StackItem {
                id: virtual_id,
                controller,
                card_id: card_id.to_string(),
                targets: vec![],
                ability_text: Some(ability_text.clone()),
                source_permanent_id: Some(source_id),
                ability_index: Some(ability_index),
                is_triggered: true,
                is_copy: false,
                chosen_x: 0,
                face_index: 0,
            });
            self.state.passes_since_stack_change = 0;
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                    object_id: virtual_id,
                    description: card_name.clone(),
                    targets: vec![],
                    ability_annotation: ability_text.clone(),
                    card_id: String::new(),
                    is_copy: false,
                })),
            });
            events.push(ev_log(format!("Triggered: {card_name} — {ability_text}")));
        }
    }
}

/// Render a color set as Cockatrice's lowercase WUBRG color string (e.g. `[White, Blue]` → "wu",
/// colorless → ""). Used to populate the token identity the relay feeds to Event_CreateToken.
fn color_string(colors: &[Color]) -> String {
    // Canonical WUBRG order regardless of input order.
    [
        (Color::White, 'w'),
        (Color::Blue, 'u'),
        (Color::Black, 'b'),
        (Color::Red, 'r'),
        (Color::Green, 'g'),
    ]
    .iter()
    .filter(|(c, _)| colors.contains(c))
    .map(|(_, ch)| ch)
    .collect()
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

/// Compute `SpellTargets` for a spell or activated ability — which objects/players `caster`
/// can legally send this set of effects at, given the current game state. Only effects that
/// need a target are considered; non-targeted effects in the same spell are ignored.
fn compute_spell_targets(
    state: &GameState,
    registry: &CardRegistry,
    caster: PlayerId,
    effects: &[SpellEffectKind],
) -> rv1::SpellTargets {
    let mut valid_permanent_ids = Vec::new();
    let mut valid_stack_ids = Vec::new();
    let mut can_target_self = false;
    let mut can_target_opponent = false;

    for obj in state.objects.values() {
        let legal = effects
            .iter()
            .filter(|e| spell_effect_kind_needs_target(e))
            .all(|e| spell_target_legality_error(state, registry, e, obj.id, caster).is_ok());
        if legal {
            match obj.zone {
                Zone::Battlefield => valid_permanent_ids.push(obj.id),
                Zone::Stack => valid_stack_ids.push(obj.id),
                _ => {}
            }
        }
    }

    for p in &state.players {
        if p.has_lost {
            continue;
        }
        let tid = p.id as ObjectId;
        let legal = effects
            .iter()
            .filter(|e| spell_effect_kind_needs_target(e))
            .all(|e| spell_target_legality_error(state, registry, e, tid, caster).is_ok());
        if legal {
            if p.id == caster {
                can_target_self = true;
            } else {
                can_target_opponent = true;
            }
        }
    }

    rv1::SpellTargets {
        valid_permanent_ids,
        valid_stack_ids,
        can_target_self,
        can_target_opponent,
    }
}

fn fill_legal(batch: &mut RuledEventBatch, eng: &GameEngine) {
    for p in &eng.state.players {
        let labels = legal_labels(eng, p.id);
        let mut valid_targets_by_hand_slot = BTreeMap::new();
        let mut valid_targets_by_ability = BTreeMap::new();

        if let Some(idx) = eng.state.player_idx(p.id) {
            // Spell targets: one entry per hand slot whose spell needs a target.
            for (slot, &oid) in eng.state.players[idx].hand.iter().enumerate() {
                let Some(obj) = eng.state.objects.get(&oid) else {
                    continue;
                };
                let Some(def) = eng.registry.get(&obj.card_id) else {
                    continue;
                };
                // CR 709/712/715: a multi-face card's halves can target different things; offer the
                // union of every face's legal targets so the UI highlights anything castable from
                // some face. The chosen face is validated server-side at cast time.
                let mut merged = rv1::SpellTargets::default();
                let mut any = false;
                for face in def.faces_iter() {
                    if face.is_land || !face.spell_effect.iter().any(spell_effect_kind_needs_target)
                    {
                        continue;
                    }
                    any = true;
                    let t =
                        compute_spell_targets(&eng.state, eng.registry, p.id, face.spell_effect);
                    for id in t.valid_permanent_ids {
                        if !merged.valid_permanent_ids.contains(&id) {
                            merged.valid_permanent_ids.push(id);
                        }
                    }
                    for id in t.valid_stack_ids {
                        if !merged.valid_stack_ids.contains(&id) {
                            merged.valid_stack_ids.push(id);
                        }
                    }
                    merged.can_target_self |= t.can_target_self;
                    merged.can_target_opponent |= t.can_target_opponent;
                }
                if any {
                    valid_targets_by_hand_slot.insert(slot as u32, merged);
                }
            }

            // Ability targets: one entry per (permanent, ability_index) pair where the
            // ability needs a target. Key encodes (permanent_oid << 32 | ability_index).
            for &poid in &eng.state.players[idx].battlefield {
                let Some(pobj) = eng.state.objects.get(&poid) else {
                    continue;
                };
                let Some(pdef) = eng.registry.get(&pobj.card_id) else {
                    continue;
                };
                for (ai, ability) in pdef.activated_abilities.iter().enumerate() {
                    if spell_effect_kind_needs_target(&ability.effect) {
                        let targets = compute_spell_targets(
                            &eng.state,
                            eng.registry,
                            p.id,
                            std::slice::from_ref(&ability.effect),
                        );
                        let key = (poid as u64) << 32 | ai as u64;
                        valid_targets_by_ability.insert(key, targets);
                    }
                }
            }
        }

        batch.legal_by_player.insert(
            p.id,
            LegalActions {
                labels,
                valid_targets_by_hand_slot,
                valid_targets_by_ability,
            },
        );
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
    // A parked tier-3 custom resolution (CR 608) must be answered before any other action.
    if let Some(pr) = &eng.state.pending_resolution {
        return if pr.deciding_player == pid {
            vec![format!("Resolve: {}", pr.prompt)]
        } else {
            vec!["Waiting: opponent making a resolution choice".into()]
        };
    }
    // Triggered ability awaiting a target must be resolved before any other action.
    if let Some(pt) = eng.state.pending_triggers.front() {
        return if pt.controller == pid {
            vec![format!("Choose target for trigger: {}", pt.ability_text)]
        } else {
            vec!["Waiting: opponent choosing trigger target".into()]
        };
    }
    // Assign combat damage sub-phase: active player must assign before anything else.
    if let Some(c) = &eng.state.combat {
        if c.blockers_declared && c.damage_assignment_needed && c.assign_combat_damage_phase {
            if pid == eng.state.active_player_id() {
                let mut out = Vec::new();
                for (&att, blks) in &c.blockers {
                    if blks.len() > 1 && !c.damage_assignments.contains_key(&att) {
                        let name = object_display_name(&eng.state, eng.registry, att);
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
            // CR 709/712/715: a multi-face card offers a label per castable face (each split
            // half / MDFC face); a single-face card has exactly one face (the flat fields).
            for face in def.faces_iter() {
                let name = face.name;
                if face.is_land {
                    if sorcery_ok && !eng.state.land_dropped_this_turn {
                        v.push(format!("Play land {name} (hand idx {i})"));
                    }
                } else if !combat_decl_lock {
                    // CR 601.3a / 702.8b: instants and flash cards use instant timing; everything
                    // else is sorcery-speed only.
                    let cast_ok = if castable_at_instant_speed(&face) {
                        instant_ok
                    } else {
                        sorcery_ok
                    };
                    if cast_ok {
                        let needs_target =
                            face.spell_effect.iter().any(spell_effect_kind_needs_target);
                        if needs_target {
                            v.push(format!("Cast {name} (hand idx {i}, target)"));
                        } else {
                            v.push(format!("Cast {name} (hand idx {i})"));
                        }
                    }
                }
            }
        } else if !combat_decl_lock && (instant_ok || sorcery_ok) {
            v.push(format!("Play unknown card (hand idx {i})"));
        }
    }
    v
}

/// Which combat damage step is being resolved (CR 510.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DamagePass {
    FirstStrike,
    Normal,
}

/// CR 510.4 participation rule. In the first-strike pass, only creatures with FirstStrike or
/// DoubleStrike assign damage. In the regular pass, creatures that did not assign during the
/// first-strike step (or weren't in it) assign damage, plus creatures that currently have
/// DoubleStrike. When no first-strike step occurred, every creature participates in the
/// regular pass (vanilla combat).
fn object_participates_in_pass(
    state: &GameState,
    registry: &tricerules_cards::CardRegistry,
    c: &CombatState,
    pass: DamagePass,
    obj_id: ObjectId,
    is_attacker: bool,
) -> bool {
    use tricerules_cards::Keyword;
    let Some(obj) = state.objects.get(&obj_id) else {
        return false;
    };
    let has_fs = obj.has_keyword(registry, Keyword::FirstStrike);
    let has_ds = obj.has_keyword(registry, Keyword::DoubleStrike);
    match pass {
        DamagePass::FirstStrike => has_fs || has_ds,
        DamagePass::Normal => {
            let was_in_first_strike = if is_attacker {
                c.first_strike_attackers.contains(&obj_id)
            } else {
                c.first_strike_blockers
                    .values()
                    .any(|bs| bs.contains(&obj_id))
            };
            !was_in_first_strike || has_ds
        }
    }
}

/// True iff any current attacker or blocker has FirstStrike or DoubleStrike — used to decide
/// whether the combat phase needs a first-strike damage substep (CR 510.4).
fn combat_needs_first_strike_step(
    state: &GameState,
    registry: &tricerules_cards::CardRegistry,
    c: &CombatState,
) -> bool {
    use tricerules_cards::Keyword;
    let has_fs_or_ds = |id: ObjectId| {
        state.objects.get(&id).is_some_and(|o| {
            o.has_keyword(registry, Keyword::FirstStrike)
                || o.has_keyword(registry, Keyword::DoubleStrike)
        })
    };
    c.attacking.iter().copied().any(has_fs_or_ds)
        || c.blockers.values().flatten().copied().any(has_fs_or_ds)
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

/// Render one [`ManaAmount`] as a brace-less symbol run for the zone view's mana-produced field
/// (e.g. `{g:1}` → `"G"`, `{c:2}` → `"CC"`, `{w:1,u:1}` → `"WU"`). Order W U B R G C is canonical.
fn mana_amount_symbols(a: &tricerules_cards::ManaAmount) -> String {
    let mut s = String::new();
    for (sym, n) in [
        ('W', a.w),
        ('U', a.u),
        ('B', a.b),
        ('R', a.r),
        ('G', a.g),
        ('C', a.c),
    ] {
        for _ in 0..n {
            s.push(sym);
        }
    }
    s
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
pub(crate) fn permanent_moved_event(
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

    // CR 400.7: a zone change creates a new game object. Remove any Single-target continuous
    // effects on this object so they don't apply if the same ObjectId is reused later.
    let leaving_battlefield = state.objects.get(&oid).map(|o| o.zone) == Some(Zone::Battlefield);
    if leaving_battlefield && z != Zone::Battlefield {
        state
            .continuous_effects
            .retain(|e| !matches!(&e.affected, AffectedScope::Single(id) if *id == oid));
    }

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
        // CR 302.6: a permanent entering the battlefield has not been controlled continuously
        // since its controller's most recent turn began, so it is summoning sick. Assert this on
        // entry rather than trusting a persisted flag — a prior bounce/leave clears transient
        // state, so a creature returned to hand and recast (or reanimated/flickered) the same turn
        // must still be sick. Haste exempts the *use* of this (checked at attack/tap time).
        if z == Zone::Battlefield {
            o.summoning_sick = true;
        }
    }
    Ok(())
}

fn destroy_permanent(state: &mut GameState, oid: ObjectId) -> Result<(), EngineError> {
    move_object_to_zone(state, oid, Zone::Graveyard)
}

/// Sacrifice a permanent (CR 701.17). Unlike destroy, sacrifice bypasses indestructible and
/// regeneration — it is always a cost, never a triggered or replacement effect that can be
/// redirected.
fn sacrifice_permanent(state: &mut GameState, oid: ObjectId) -> Result<(), EngineError> {
    move_object_to_zone(state, oid, Zone::Graveyard)
}

/// Pay `cost` from `player_idx`'s mana pool. `x_value` is the chosen value for any `{X}` pip
/// (CR 107.3); each `{X}` is paid as that many generic mana. Pass `0` for costs without `{X}`.
/// Pool counts indexed `[W, U, B, R, G, C]`. `C` is the colorless slot.
type PoolVec = [u32; 6];
const POOL_C: usize = 5;

/// Index into a [`PoolVec`] for a colored pip.
fn color_index(c: ColorPip) -> usize {
    match c {
        ColorPip::W => 0,
        ColorPip::U => 1,
        ColorPip::B => 2,
        ColorPip::R => 3,
        ColorPip::G => 4,
    }
}

/// A flexible pip resolved against the pool after fixed colored/colorless demands are met.
enum FlexPip {
    /// Pay one mana of either color (hybrid `{G/U}`).
    Hybrid(ColorPip, ColorPip),
    /// Pay one mana of the color, or `n` generic (mono-hybrid `{2/W}`).
    Mono(u32, ColorPip),
    /// Pay one mana of the color (Phyrexian `{B/P}` paid with mana, not life).
    Color(ColorPip),
}

/// Backtracking solve: can `flex` plus `generic` generic mana be paid from `pool`? Returns the
/// leftover pool on success. Bounded by 2^(flexible pips), which is tiny in practice (a cost has
/// only a handful of pips); the search exists to avoid greedy dead-ends like `{G/U}{G}` paid from
/// one G and one U (CR 107.4 — hybrid payment is a genuine choice, not a fixed drain).
fn solve_flex(pool: PoolVec, flex: &[FlexPip], idx: usize, generic: u32) -> Option<PoolVec> {
    if idx == flex.len() {
        // Pay generic from whatever remains, colorless first (preserves prior drain order).
        let mut p = pool;
        let mut g = generic;
        for &i in &[POOL_C, 0, 1, 2, 3, 4] {
            let t = g.min(p[i]);
            p[i] -= t;
            g -= t;
        }
        return (g == 0).then_some(p);
    }
    match &flex[idx] {
        FlexPip::Hybrid(a, b) => {
            for &c in &[*a, *b] {
                let i = color_index(c);
                if pool[i] > 0 {
                    let mut p = pool;
                    p[i] -= 1;
                    if let Some(r) = solve_flex(p, flex, idx + 1, generic) {
                        return Some(r);
                    }
                }
            }
            None
        }
        FlexPip::Color(c) => {
            let i = color_index(*c);
            if pool[i] == 0 {
                return None;
            }
            let mut p = pool;
            p[i] -= 1;
            solve_flex(p, flex, idx + 1, generic)
        }
        FlexPip::Mono(n, c) => {
            // Prefer paying with the color (cheaper in mana count); fall back to `n` generic.
            let i = color_index(*c);
            if pool[i] > 0 {
                let mut p = pool;
                p[i] -= 1;
                if let Some(r) = solve_flex(p, flex, idx + 1, generic) {
                    return Some(r);
                }
            }
            solve_flex(pool, flex, idx + 1, generic + n)
        }
    }
}

fn pay_mana(
    state: &mut GameState,
    player_idx: usize,
    cost: &ManaCost,
    x_value: u32,
    flex_payments: &[rv1::FlexPipPayment],
) -> Result<(), EngineError> {
    // Mana must already be in the player's pool (added via AddManaToPool / land taps).
    // The engine never auto-taps lands — the client is responsible for sending AddManaToPool
    // before casting a spell or activating an ability.
    if player_idx != state.priority_idx {
        return Err(EngineError::Illegal(
            "only priority player can pay mana for spells",
        ));
    }
    // Pip indices the player chose to pay with life (CR 107.4f). Only meaningful on Phyrexian pips.
    let life_pips: HashSet<usize> = flex_payments
        .iter()
        .filter(|fp| fp.pay_life)
        .map(|fp| fp.pip_index as usize)
        .collect();

    let mut need_color: PoolVec = [0; 6]; // fixed W U B R G demand + colorless in slot 5
    let mut need_generic = 0u32; // {N}/{X}: payable by any mana
    let mut life_cost = 0u32; // Phyrexian pips paid with life
    let mut flex: Vec<FlexPip> = Vec::new();
    for (i, pip) in cost.pips.iter().enumerate() {
        match pip {
            ManaSymbol::W => need_color[color_index(ColorPip::W)] += 1,
            ManaSymbol::U => need_color[color_index(ColorPip::U)] += 1,
            ManaSymbol::B => need_color[color_index(ColorPip::B)] += 1,
            ManaSymbol::R => need_color[color_index(ColorPip::R)] += 1,
            ManaSymbol::G => need_color[color_index(ColorPip::G)] += 1,
            ManaSymbol::C => need_color[POOL_C] += 1,
            ManaSymbol::Generic(n) => need_generic += n,
            // CR 107.3b: each {X} pip is paid as `x_value` generic mana. One X is shared across
            // all {X} pips in a cost (CR 107.3c) — the caller passes the single chosen value.
            ManaSymbol::X => need_generic += x_value,
            ManaSymbol::Hybrid(a, b) => flex.push(FlexPip::Hybrid(*a, *b)),
            ManaSymbol::MonoHybrid(n, c) => flex.push(FlexPip::Mono(*n, *c)),
            ManaSymbol::Phyrexian(c) => {
                if life_pips.contains(&i) {
                    life_cost += 2;
                } else {
                    flex.push(FlexPip::Color(*c));
                }
            }
        }
    }

    // CR 119.4: a player can pay life only if they have at least that much. Checked before any
    // mana is drained so a failed payment leaves life untouched (costs are paid atomically,
    // CR 601.2h). The actual deduction happens only after the mana solve succeeds.
    if life_cost > 0 && state.players[player_idx].life < life_cost as i32 {
        return Err(EngineError::Illegal(
            "not enough life to pay Phyrexian cost",
        ));
    }

    let pool = &state.players[player_idx].mana_pool;
    let mut working: PoolVec = [
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    ];
    // Fixed colored and colorless demands must be met exactly from matching mana.
    for i in 0..6 {
        if working[i] < need_color[i] {
            return Err(EngineError::Illegal(
                "not enough mana in pool; tap your lands first",
            ));
        }
        working[i] -= need_color[i];
    }
    let Some(remaining) = solve_flex(working, &flex, 0, need_generic) else {
        return Err(EngineError::Illegal(
            "not enough mana in pool; tap your lands first",
        ));
    };

    // Commit: write the solved pool back, then deduct any Phyrexian life.
    let pool = &mut state.players[player_idx].mana_pool;
    pool.white = remaining[0];
    pool.blue = remaining[1];
    pool.black = remaining[2];
    pool.red = remaining[3];
    pool.green = remaining[4];
    pool.colorless = remaining[POOL_C];
    state.players[player_idx].life -= life_cost as i32;
    Ok(())
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

/// CR 702.16 / CR 702.18: returns false when `tid` is a permanent that the `caster` cannot
/// legally target due to Shroud or Hexproof. Players are never shielded by these keywords.
fn object_targetable_by(
    state: &GameState,
    registry: &CardRegistry,
    tid: ObjectId,
    caster: PlayerId,
) -> bool {
    let Some(obj) = state.objects.get(&tid) else {
        return true; // object gone — legality checked elsewhere
    };
    let Some(def) = registry.get(&obj.card_id) else {
        return true;
    };
    if def.keywords.contains(&Keyword::Shroud) {
        return false;
    }
    if def.keywords.contains(&Keyword::Hexproof) && obj.owner != caster {
        return false;
    }
    true
}

/// Legality of a single target against a [`TargetFilter`].
/// `caster` is needed only to enforce the opponent-only restriction.
/// True if `oid` is a battlefield permanent selected by a mass effect's `kind` filter
/// (DestroyAll / DamageAll). Unlike [`target_filter_legal`] this is **not** targeting: it
/// ignores hexproof/shroud (CR 702.11e — untargeted effects affect them normally) and only
/// honors the object kinds and characteristic constraints the filter carries.
fn object_matches_mass_filter(
    state: &GameState,
    registry: &CardRegistry,
    oid: ObjectId,
    filter: &TargetFilter,
) -> bool {
    let Some(o) = state.objects.get(&oid) else {
        return false;
    };
    if o.zone != Zone::Battlefield {
        return false;
    }
    let kind_ok = match filter.kind {
        TargetKind::Creature => o.is_creature(registry),
        TargetKind::AnyPermanent => true,
        // Player / AnyTarget / Self_ kinds are rejected at registry load for mass effects.
        _ => false,
    };
    if !kind_ok {
        return false;
    }
    if filter.not_artifact
        && registry
            .get(&o.card_id)
            .map(|d| d.is_artifact)
            .unwrap_or(false)
    {
        return false;
    }
    if let Some(tapped_req) = filter.tapped {
        if o.tapped != tapped_req {
            return false;
        }
    }
    true
}

/// Collect every battlefield permanent matching a mass-effect filter, in deterministic
/// player-then-battlefield order (no HashMap iteration, so replays stay reproducible).
fn battlefield_objects_matching(
    state: &GameState,
    registry: &CardRegistry,
    filter: &TargetFilter,
) -> Vec<ObjectId> {
    let mut out = Vec::new();
    for p in &state.players {
        for &oid in &p.battlefield {
            if object_matches_mass_filter(state, registry, oid, filter) {
                out.push(oid);
            }
        }
    }
    out
}

fn target_filter_legal(
    state: &GameState,
    registry: &CardRegistry,
    filter: &TargetFilter,
    tid: ObjectId,
    caster: PlayerId,
) -> bool {
    let kind_ok = match filter.kind {
        TargetKind::AnyTarget => damage_spell_target_legal(state, registry, tid),
        TargetKind::Creature => destroy_spell_target_legal(state, registry, tid),
        TargetKind::AnyPlayer => player_target_legal(state, tid),
        TargetKind::OpponentPlayer => player_target_legal(state, tid) && tid as i32 != caster,
        TargetKind::AnyPermanent => any_battlefield_permanent_target_legal(state, tid),
        // `Self_` is auto-bound to the ability's source, never a chosen target (CR 115), so it
        // is never legal to *pick*. The engine binds it directly at resolution.
        TargetKind::Self_ => false,
    };
    if !kind_ok {
        return false;
    }
    // Characteristic filters — only apply to non-player targets.
    if !filter.is_player() {
        if !object_targetable_by(state, registry, tid, caster) {
            return false;
        }
        if filter.not_artifact {
            if let Some(obj) = state.objects.get(&tid) {
                if registry
                    .get(&obj.card_id)
                    .map(|d| d.is_artifact)
                    .unwrap_or(false)
                {
                    return false;
                }
            }
        }
        if let Some(tapped_req) = filter.tapped {
            match state.objects.get(&tid) {
                Some(obj) if obj.tapped != tapped_req => return false,
                None => return false,
                _ => {}
            }
        }
    }
    true
}

/// Human-readable counter name for game-log messages (CR 122.1).
fn counter_label(kind: CounterKind) -> &'static str {
    match kind {
        CounterKind::PlusOnePlusOne => "+1/+1",
        CounterKind::MinusOneMinusOne => "-1/-1",
    }
}

/// CR 608.2b-style: if any targeted effect has no legal target, the whole spell fizzles.
/// (With a shared single-target list this is equivalent to "all targets illegal".)
fn spell_has_no_legal_targets_at_resolution(
    state: &GameState,
    registry: &CardRegistry,
    effects: &[SpellEffectKind],
    targets: &[ObjectId],
    caster: PlayerId,
) -> bool {
    effects.iter().any(|effect| {
        if !spell_effect_kind_needs_target(effect) {
            return false; // untargeted effects never fizzle
        }
        let Some(&tid) = targets.first() else {
            return true; // needs target but none provided
        };
        !effect_target_legal_at_resolution(state, registry, effect, tid, caster)
    })
}

/// Returns true if `tid` is a legal target for `effect` at resolution time.
fn effect_target_legal_at_resolution(
    state: &GameState,
    registry: &CardRegistry,
    effect: &SpellEffectKind,
    tid: ObjectId,
    caster: PlayerId,
) -> bool {
    match effect {
        SpellEffectKind::DamageTarget { target, .. }
        | SpellEffectKind::TargetPlayerGainsLife { target, .. }
        | SpellEffectKind::TargetPlayerLosesLife { target, .. }
        | SpellEffectKind::MillTargetPlayer { target, .. }
        | SpellEffectKind::TapTarget { target } => {
            target_filter_legal(state, registry, target, tid, caster)
        }
        SpellEffectKind::DestroyTarget { target }
        | SpellEffectKind::PumpTarget { target, .. }
        | SpellEffectKind::PutCounters { target, .. } => {
            target_filter_legal(state, registry, target, tid, caster)
        }
        SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand => {
            destroy_spell_target_legal(state, registry, tid)
                && object_targetable_by(state, registry, tid, caster)
        }
        SpellEffectKind::ReturnTargetPermanentToHand => {
            any_battlefield_permanent_target_legal(state, tid)
                && object_targetable_by(state, registry, tid, caster)
        }
        // CR 115.2 / 707.10b: counter and copy effects target *spells* on the stack, not
        // activated/triggered abilities. (Both abilities and copies carry `ability_text` /
        // `is_copy`; a copy is itself a spell, so it stays a legal target for either.)
        SpellEffectKind::CounterTargetSpell | SpellEffectKind::CopyTargetSpell { .. } => state
            .stack
            .iter()
            .any(|s| s.id == tid && s.ability_text.is_none()),
        _ => true,
    }
}

fn spell_effect_kind_needs_target(kind: &SpellEffectKind) -> bool {
    match kind {
        // A `Self_`-filtered pump or counter-placement is auto-bound to its source (CR 115) — it
        // takes no chosen target and prompts nobody; any other filter requires a selected target.
        SpellEffectKind::PumpTarget { target, .. }
        | SpellEffectKind::PutCounters { target, .. } => !matches!(target.kind, TargetKind::Self_),
        SpellEffectKind::DamageTarget { .. }
        | SpellEffectKind::DestroyTarget { .. }
        | SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand
        | SpellEffectKind::ReturnTargetPermanentToHand
        | SpellEffectKind::TargetPlayerGainsLife { .. }
        | SpellEffectKind::TargetPlayerLosesLife { .. }
        | SpellEffectKind::MillTargetPlayer { .. }
        | SpellEffectKind::TapTarget { .. }
        | SpellEffectKind::CounterTargetSpell
        | SpellEffectKind::CopyTargetSpell { .. } => true,
        _ => false,
    }
}

/// Validate targets for a `SpellEffectKind` directly (used by ability activation/trigger target selection).
fn validate_effect_targets(
    state: &GameState,
    registry: &CardRegistry,
    caster: PlayerId,
    effect: &SpellEffectKind,
    targets: &[rv1::TargetRef],
) -> Result<(), EngineError> {
    match effect {
        SpellEffectKind::DestroyTarget { target: filter } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(state, registry, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal(
                    "target must be a creature on the battlefield",
                ));
            }
        }
        SpellEffectKind::TapTarget { target: filter } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(state, registry, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal(
                    "target must be a permanent on the battlefield",
                ));
            }
        }
        SpellEffectKind::DamageTarget { target: filter, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(state, registry, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal("illegal target for damage effect"));
            }
        }
        SpellEffectKind::PumpTarget { target: filter, .. }
        | SpellEffectKind::PutCounters { target: filter, .. } => {
            // `Self_` pumps / counter placements are auto-bound and take no chosen target.
            if matches!(filter.kind, TargetKind::Self_) {
                if !targets.is_empty() {
                    return Err(EngineError::Illegal("this effect takes no targets"));
                }
            } else {
                if targets.len() != 1 {
                    return Err(EngineError::Illegal("requires exactly one target"));
                }
                if !target_filter_legal(state, registry, filter, targets[0].object_id, caster) {
                    return Err(EngineError::Illegal(
                        "target must be a creature on the battlefield",
                    ));
                }
            }
        }
        SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one creature target"));
            }
            if !destroy_spell_target_legal(state, registry, targets[0].object_id) {
                return Err(EngineError::Illegal(
                    "target must be a creature on the battlefield",
                ));
            }
            if !object_targetable_by(state, registry, targets[0].object_id, caster) {
                return Err(EngineError::Illegal("target has hexproof or shroud"));
            }
        }
        SpellEffectKind::ReturnTargetPermanentToHand => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal(
                    "requires exactly one permanent target",
                ));
            }
            if !any_battlefield_permanent_target_legal(state, targets[0].object_id) {
                return Err(EngineError::Illegal(
                    "target must be a permanent on the battlefield",
                ));
            }
            if !object_targetable_by(state, registry, targets[0].object_id, caster) {
                return Err(EngineError::Illegal("target has hexproof or shroud"));
            }
        }
        SpellEffectKind::TargetPlayerGainsLife { target: filter, .. }
        | SpellEffectKind::TargetPlayerLosesLife { target: filter, .. }
        | SpellEffectKind::MillTargetPlayer { target: filter, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one player target"));
            }
            if !target_filter_legal(state, registry, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal("target must be a player in the game"));
            }
            if matches!(filter.kind, TargetKind::OpponentPlayer)
                && targets[0].object_id as i32 == caster
            {
                return Err(EngineError::Illegal("cannot target yourself"));
            }
        }
        // Non-targeted effects require no targets.
        SpellEffectKind::Draw { .. }
        | SpellEffectKind::GainLife { .. }
        | SpellEffectKind::EachOpponentLosesLifeYouGainEqual { .. }
        // Counter/copy target *spells*; they are never put on an ability, so this ability-only
        // validator only needs them present for exhaustiveness (spell targets go through
        // `spell_target_legality_error`).
        | SpellEffectKind::CounterTargetSpell
        | SpellEffectKind::CopyTargetSpell { .. }
        | SpellEffectKind::DestroyAll { .. }
        | SpellEffectKind::DamageAll { .. }
        | SpellEffectKind::CreateTokens { .. }
        // CR 605.1a: a mana ability is untargeted by definition.
        | SpellEffectKind::ProduceMana { .. }
        | SpellEffectKind::None => {
            if !targets.is_empty() {
                return Err(EngineError::Illegal("this effect takes no targets"));
            }
        }
    }
    Ok(())
}

fn validate_spell_targets(
    state: &GameState,
    registry: &CardRegistry,
    caster: PlayerId,
    effects: &[SpellEffectKind],
    targets: &[rv1::TargetRef],
) -> Result<(), EngineError> {
    let needs_target = effects.iter().any(spell_effect_kind_needs_target);
    if needs_target {
        if targets.len() != 1 {
            return Err(EngineError::Illegal("spell requires exactly one target"));
        }
        let tid = targets[0].object_id;
        for effect in effects {
            if !spell_effect_kind_needs_target(effect) {
                continue;
            }
            spell_target_legality_error(state, registry, effect, tid, caster)?;
        }
    } else if !targets.is_empty() {
        return Err(EngineError::Illegal("this spell takes no targets"));
    }
    Ok(())
}

/// Returns `Err` with a specific human-readable message when `tid` is not a legal target for `effect`.
fn spell_target_legality_error(
    state: &GameState,
    registry: &CardRegistry,
    effect: &SpellEffectKind,
    tid: ObjectId,
    caster: PlayerId,
) -> Result<(), EngineError> {
    match effect {
        // Filter-based targeted effects share one legality path; the filter carries any
        // characteristic restriction (creature/player, `tapped`, `not_artifact`, hexproof/shroud).
        SpellEffectKind::DestroyTarget { target: filter }
        | SpellEffectKind::DamageTarget { target: filter, .. }
        | SpellEffectKind::TapTarget { target: filter }
        | SpellEffectKind::PumpTarget { target: filter, .. }
        | SpellEffectKind::PutCounters { target: filter, .. }
            if !target_filter_legal(state, registry, filter, tid, caster) =>
        {
            return Err(EngineError::Illegal(
                "target must be a creature or player on the battlefield",
            ));
        }
        SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand
            if !destroy_spell_target_legal(state, registry, tid) =>
        {
            return Err(EngineError::Illegal(
                "target must be a creature on the battlefield",
            ));
        }
        SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand
            if !object_targetable_by(state, registry, tid, caster) =>
        {
            return Err(EngineError::Illegal("target has hexproof or shroud"));
        }
        SpellEffectKind::ReturnTargetPermanentToHand
            if !any_battlefield_permanent_target_legal(state, tid) =>
        {
            return Err(EngineError::Illegal(
                "target must be a permanent on the battlefield",
            ));
        }
        SpellEffectKind::ReturnTargetPermanentToHand
            if !object_targetable_by(state, registry, tid, caster) =>
        {
            return Err(EngineError::Illegal("target has hexproof or shroud"));
        }
        SpellEffectKind::TargetPlayerGainsLife { target: filter, .. }
        | SpellEffectKind::TargetPlayerLosesLife { target: filter, .. }
        | SpellEffectKind::MillTargetPlayer { target: filter, .. } => {
            if !player_target_legal(state, tid) {
                return Err(EngineError::Illegal("target must be a player in the game"));
            }
            if matches!(filter.kind, TargetKind::OpponentPlayer) && tid as i32 == caster {
                return Err(EngineError::Illegal(
                    "target must be an opponent (cannot target yourself)",
                ));
            }
        }
        // CR 115.2 / 707.10b: counter and copy effects target spells, not abilities.
        SpellEffectKind::CounterTargetSpell | SpellEffectKind::CopyTargetSpell { .. }
            if !state
                .stack
                .iter()
                .any(|s| s.id == tid && s.ability_text.is_none()) =>
        {
            return Err(EngineError::Illegal("target must be a spell on the stack"));
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod mana_payment_tests {
    use super::*;

    /// Build a 2-player engine and hand priority to player 0 so `pay_mana`'s priority gate passes.
    fn engine_with_priority() -> GameEngine {
        let mut e = GameEngine::new_with_default_decks(1, &[0, 1], 20).expect("new");
        e.state.priority_idx = 0;
        e
    }

    #[test]
    fn multi_digit_generic_paid_from_pool() {
        // Regression for the old per-char parser: "{15}" must consume 15, not 6.
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 15;
        let cost = ManaCost {
            pips: vec![ManaSymbol::Generic(15)],
        };
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    }

    #[test]
    fn insufficient_generic_mana_rejected() {
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 14;
        let cost = ManaCost {
            pips: vec![ManaSymbol::Generic(15)],
        };
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 0, &[]),
            Err(EngineError::Illegal(_))
        ));
    }

    #[test]
    fn colorless_pip_requires_colorless_mana() {
        // {C} cannot be paid with colored mana (unlike generic).
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 5;
        let cost = ManaCost {
            pips: vec![ManaSymbol::C],
        };
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 0, &[]),
            Err(EngineError::Illegal(_))
        ));
        e.state.players[0].mana_pool.colorless = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
    }

    #[test]
    fn x_cost_pays_chosen_value_as_generic() {
        // CR 107.3b: {X}{R} with X=4 needs 4 generic + 1 red.
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 4;
        e.state.players[0].mana_pool.red = 1;
        let cost = ManaCost {
            pips: vec![ManaSymbol::X, ManaSymbol::R],
        };
        assert!(pay_mana(&mut e.state, 0, &cost, 4, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.colorless, 0);
        assert_eq!(e.state.players[0].mana_pool.red, 0);
    }

    #[test]
    fn x_zero_pays_only_fixed_pips() {
        // CR 107.3: X=0 is a legal choice; only the {R} must be paid.
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 1;
        let cost = ManaCost {
            pips: vec![ManaSymbol::X, ManaSymbol::R],
        };
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
    }

    #[test]
    fn insufficient_mana_for_chosen_x_rejected() {
        // Choosing X larger than the pool can cover fails cleanly (no partial payment).
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 3;
        e.state.players[0].mana_pool.red = 1;
        let cost = ManaCost {
            pips: vec![ManaSymbol::X, ManaSymbol::R],
        };
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 4, &[]),
            Err(EngineError::Illegal(_))
        ));
    }

    #[test]
    fn hybrid_pip_paid_by_either_color() {
        // {G/U} payable by green alone or by blue alone (CR 107.4d).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Hybrid(ColorPip::G, ColorPip::U)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.green = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.green, 0);

        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.blue = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.blue, 0);
    }

    #[test]
    fn hybrid_solve_avoids_greedy_dead_end() {
        // {G/U}{G} with one G + one U: the {G/U} must take U so the {G} can take G (CR 107.4).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Hybrid(ColorPip::G, ColorPip::U), ManaSymbol::G],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.green = 1;
        e.state.players[0].mana_pool.blue = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.green, 0);
        assert_eq!(e.state.players[0].mana_pool.blue, 0);
    }

    #[test]
    fn mono_hybrid_paid_by_generic_when_color_absent() {
        // {2/W} payable by two generic when no white is available (CR 107.4e).
        let cost = ManaCost {
            pips: vec![ManaSymbol::MonoHybrid(2, ColorPip::W)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 2;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.red, 0);

        // ...or by one white (the cheaper alternative is preferred, leaving generic mana spare).
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.white = 1;
        e.state.players[0].mana_pool.red = 2;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.white, 0);
        assert_eq!(e.state.players[0].mana_pool.red, 2);
    }

    #[test]
    fn mono_hybrid_rejected_with_one_generic() {
        // One spare mana can't cover the {2/...} generic alternative.
        let cost = ManaCost {
            pips: vec![ManaSymbol::MonoHybrid(2, ColorPip::W)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 1;
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 0, &[]),
            Err(EngineError::Illegal(_))
        ));
    }

    #[test]
    fn phyrexian_paid_by_mana() {
        // {B/P} with no life flag must be paid by black mana (CR 107.4f).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Phyrexian(ColorPip::B)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.black = 1;
        let life = e.state.players[0].life;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.black, 0);
        assert_eq!(e.state.players[0].life, life); // no life paid
    }

    #[test]
    fn phyrexian_paid_by_life() {
        // {B/P} with pay_life on pip 0 deducts 2 life and touches no mana (CR 107.4f).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Phyrexian(ColorPip::B)],
        };
        let mut e = engine_with_priority();
        let life = e.state.players[0].life;
        let flex = [rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: true,
        }];
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &flex).is_ok());
        assert_eq!(e.state.players[0].life, life - 2);
    }

    #[test]
    fn phyrexian_life_rejected_when_insufficient_life() {
        // CR 119.4: can't pay 2 life with only 1 (and no mana to fall back on).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Phyrexian(ColorPip::B)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].life = 1;
        let flex = [rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: true,
        }];
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 0, &flex),
            Err(EngineError::Illegal(_))
        ));
        assert_eq!(e.state.players[0].life, 1); // unchanged on failure
    }
}
