//! Core rules processing (vanilla core — simplified combat & mana).

use crate::state::{
    GameObject, GamePhase, GameState, ObjectId, PlayerId, PlayerState, StackItem, Zone,
};
use prost::Message;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use std::collections::HashMap;
use thiserror::Error;
use tricerules_cards::primitives::{spell_effect_from_key, SpellEffectKind};
use tricerules_cards::CardRegistry;
use tricerules_proto::ruled::v1::{
    IpcResponse, LegalActions, RuledCommand, RuledEvent, RuledEventBatch,
};
use tricerules_proto::ruled::v1 as rv1;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("unknown player {0}")]
    UnknownPlayer(PlayerId),
    #[error("illegal command: {0}")]
    Illegal(&'static str),
    #[error("missing card data {0}")]
    MissingCard(String),
}

pub struct GameEngine {
    pub state: GameState,
    registry: CardRegistry,
}

impl GameEngine {
    pub fn new(seed: u64, player_ids: &[PlayerId], starting_life: i32) -> Result<Self, EngineError> {
        let registry = CardRegistry::from_embedded().map_err(|_| EngineError::Illegal("bad registry"))?;
        let mut objects = HashMap::new();
        let mut next_object_id: ObjectId = 1;
        let mut players = Vec::new();

        for (i, &pid) in player_ids.iter().enumerate() {
            let mut p = PlayerState::new(pid, starting_life);
            let deck_list = default_deck_list(i);
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
                    },
                );
                p.library.push_back(oid);
            }
            let mut rng = StdRng::seed_from_u64(seed.wrapping_add(i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let mut lib: Vec<ObjectId> = p.library.iter().copied().collect();
            lib.shuffle(&mut rng);
            p.library = lib.into_iter().collect();
            for _ in 0..7 {
                draw_card(&mut p, &mut objects)?;
            }
            players.push(p);
        }

        let state = GameState {
            seed,
            players,
            objects,
            stack: Vec::new(),
            priority_idx: 0,
            active_player_idx: 0,
            phase: GamePhase::Main1,
            turn: 1,
            next_object_id,
            command_index: 0,
            passes_since_stack_change: 0,
        };
        let mut eng = GameEngine { state, registry };
        eng.apply_sbas();
        Ok(eng)
    }

    pub fn apply_command(
        &mut self,
        player: PlayerId,
        cmd: &RuledCommand,
    ) -> Result<RuledEventBatch, EngineError> {
        self.state.command_index += 1;
        use rv1::ruled_command::Cmd;
        match cmd.cmd.as_ref() {
            Some(Cmd::PassPriority(_)) => self.pass_priority(player),
            Some(Cmd::CastSpell(cs)) => self.cast_spell(player, cs.hand_card_index as usize, &cs.targets),
            Some(Cmd::PlayLand(pl)) => self.play_land(player, pl.hand_card_index as usize),
            Some(Cmd::Mulligan(_)) => Ok(empty_batch_with_legal(self)),
            Some(Cmd::Concede(_)) => Ok(self.concede_batch(player)),
            None => Err(EngineError::Illegal("empty command")),
        }
    }

    fn concede_batch(&self, player: PlayerId) -> RuledEventBatch {
        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!("Player {player} conceded")));
        let mut b = batch;
        fill_legal(&mut b, self);
        b
    }

    fn pass_priority(&mut self, player: PlayerId) -> Result<RuledEventBatch, EngineError> {
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        let n = self.state.players.len() as u32;
        let mut events = vec![rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::PriorityChanged(rv1::PriorityChanged {
                player_id: player,
            })),
        }];

        self.state.passes_since_stack_change += 1;
        self.state.priority_idx = (self.state.priority_idx + 1) % self.state.players.len();

        if !self.state.stack.is_empty() && self.state.passes_since_stack_change >= n {
            self.resolve_top_of_stack(&mut events)?;
            self.state.passes_since_stack_change = 0;
        } else if self.state.stack.is_empty() && self.state.passes_since_stack_change >= n {
            // End of step — simplified: give priority back to active player
            self.state.passes_since_stack_change = 0;
            self.state.priority_idx = self.state.active_player_idx;
            events.push(ev_phase("Step end (simplified)"));
        }

        self.apply_sbas();
        let mut batch = RuledEventBatch {
            events,
            legal_by_player: Default::default(),
        };
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    fn resolve_top_of_stack(&mut self, events: &mut Vec<rv1::RuledEvent>) -> Result<(), EngineError> {
        let top = self.state.stack.pop().ok_or(EngineError::Illegal("empty stack"))?;
        let controller = top.controller;
        let card_id = top.card_id.clone();
        let targets = top.targets.clone();

        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::StackResolved(rv1::StackResolved {
                object_id: top.id,
            })),
        });

        move_object_to_zone(&mut self.state, top.id, Zone::Graveyard)?;

        let effect = self
            .registry
            .get(&card_id)
            .and_then(|c| c.spell_effect.as_ref())
            .map(|s| spell_effect_from_key(s))
            .unwrap_or(SpellEffectKind::None);

        match effect {
            SpellEffectKind::DealDamage { amount } => {
                if let Some(&tid) = targets.first() {
                    if let Some(t) = self.state.objects.get_mut(&tid) {
                        if t.is_creature(&self.registry) {
                            t.damage += amount;
                        }
                    }
                }
            }
            SpellEffectKind::Draw { count } => {
                let idx = self.state.player_idx(controller).unwrap();
                for _ in 0..count {
                    draw_card(&mut self.state.players[idx], &mut self.state.objects)?;
                }
            }
            SpellEffectKind::PumpTarget { power, toughness } => {
                if let Some(&tid) = targets.first() {
                    if let Some(t) = self.state.objects.get_mut(&tid) {
                        let p = t.power.unwrap_or(0) as i32 + power;
                        let tt = t.toughness.unwrap_or(0) as i32 + toughness;
                        t.power = Some(p.max(0) as u32);
                        t.toughness = Some(tt.max(0) as u32);
                    }
                }
            }
            SpellEffectKind::DestroyTarget => {
                if let Some(&tid) = targets.first() {
                    destroy_permanent(&mut self.state, tid)?;
                }
            }
            SpellEffectKind::CounterTargetSpell => {
                if let Some(&tid) = targets.first() {
                    if let Some(pos) = self.state.stack.iter().position(|s| s.id == tid) {
                        let st = self.state.stack.remove(pos);
                        move_object_to_zone(&mut self.state, st.id, Zone::Graveyard)?;
                        events.push(ev_log("Countered spell".into()));
                    }
                }
            }
            SpellEffectKind::None => {}
        }
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
        let idx = self.state.player_idx(player).ok_or(EngineError::UnknownPlayer(player))?;
        let oid = *self
            .state
            .players[idx]
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
        pay_mana_simple(&mut self.state, idx, &def.mana_cost)?;

        self.state.players[idx].hand.retain(|&x| x != oid);
        let trefs: Vec<ObjectId> = targets.iter().map(|t| t.object_id).collect();

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
        // Next player gets priority (APNAP simplification)
        self.state.priority_idx = (idx + 1) % self.state.players.len();

        let def_name = def.name.clone();
        let mut batch = RuledEventBatch::default();
        batch.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                object_id: oid,
                description: def_name,
            })),
        });
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    fn play_land(&mut self, player: PlayerId, hand_idx: usize) -> Result<RuledEventBatch, EngineError> {
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        let idx = self.state.player_idx(player).ok_or(EngineError::UnknownPlayer(player))?;
        let oid = *self
            .state
            .players[idx]
            .hand
            .get(hand_idx)
            .ok_or(EngineError::Illegal("bad hand index"))?;
        let card_id = self.state.objects.get(&oid).unwrap().card_id.clone();
        let def = self.registry.get(&card_id).unwrap();
        if !def.is_land {
            return Err(EngineError::Illegal("not a land"));
        }
        self.state.players[idx].hand.retain(|&x| x != oid);
        self.state.players[idx].battlefield.push(oid);
        if let Some(o) = self.state.objects.get_mut(&oid) {
            o.zone = Zone::Battlefield;
        }
        self.state.passes_since_stack_change = 0;
        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!("Played {}", def.name)));
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    fn apply_sbas(&mut self) {
        let mut to_destroy = Vec::new();
        for (&id, o) in &self.state.objects {
            if o.zone == Zone::Battlefield {
                if let Some(t) = o.toughness {
                    if o.damage >= t {
                        to_destroy.push(id);
                    }
                }
            }
        }
        for id in to_destroy {
            let _ = destroy_permanent(&mut self.state, id);
        }
    }

    pub fn initial_response_batch(&self) -> RuledEventBatch {
        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_phase("Main1"));
        batch.events.push(ev_log(format!(
            "Game start — active P{}, priority P{}",
            self.state.active_player_id(),
            self.state.priority_player_id()
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
}

fn default_deck_list(player_index: usize) -> Vec<String> {
    if player_index == 0 {
        let mut d: Vec<String> = std::iter::repeat("mountain".into()).take(20).collect();
        d.extend(std::iter::repeat("lightning_bolt".into()).take(20));
        d.extend(std::iter::repeat("grizzly_bears".into()).take(20));
        d.truncate(60);
        d
    } else {
        let mut d: Vec<String> = std::iter::repeat("forest".into()).take(20).collect();
        d.extend(std::iter::repeat("giant_growth".into()).take(20));
        d.extend(std::iter::repeat("counterspell".into()).take(20));
        d.truncate(60);
        d
    }
}

fn empty_batch_with_legal(eng: &GameEngine) -> RuledEventBatch {
    let mut b = RuledEventBatch::default();
    fill_legal(&mut b, eng);
    b
}

fn fill_legal(batch: &mut RuledEventBatch, eng: &GameEngine) {
    for p in &eng.state.players {
        let labels = legal_labels(eng, p.id);
        batch
            .legal_by_player
            .insert(p.id, LegalActions { labels });
    }
}

fn legal_labels(eng: &GameEngine, pid: PlayerId) -> Vec<String> {
    let mut v = vec!["Pass priority".into()];
    if eng.state.priority_player_id() != pid {
        return v;
    }
    let idx = match eng.state.player_idx(pid) {
        Some(i) => i,
        None => return v,
    };
    for (i, &oid) in eng.state.players[idx].hand.iter().enumerate() {
        let cid = &eng.state.objects.get(&oid).unwrap().card_id;
        let name = eng
            .registry
            .get(cid)
            .map(|c| c.name.as_str())
            .unwrap_or("?");
        v.push(format!("Cast {name} (hand idx {i})"));
        v.push(format!("Play land {name} (hand idx {i})"));
    }
    v
}

fn ev_phase(name: &str) -> RuledEvent {
    RuledEvent {
        ev: Some(rv1::ruled_event::Ev::PhaseChanged(rv1::PhaseChanged {
            phase: name.into(),
            active_player_id: 0,
        })),
    }
}

fn ev_log(text: String) -> RuledEvent {
    RuledEvent {
        ev: Some(rv1::ruled_event::Ev::Log(rv1::LogMessage { text })),
    }
}

fn draw_card(p: &mut PlayerState, objects: &mut HashMap<ObjectId, GameObject>) -> Result<(), EngineError> {
    let oid = p.library.pop_front().ok_or(EngineError::Illegal("library empty"))?;
    p.hand.push(oid);
    if let Some(o) = objects.get_mut(&oid) {
        o.zone = Zone::Hand;
    }
    Ok(())
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
    match z {
        Zone::Graveyard => p.graveyard.push(oid),
        Zone::Hand => p.hand.push(oid),
        Zone::Battlefield => p.battlefield.push(oid),
        Zone::Library => p.library.push_back(oid),
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

fn pay_mana_simple(state: &mut GameState, player_idx: usize, cost: &str) -> Result<(), EngineError> {
    let mut need_r = 0u32;
    let mut need_g = 0u32;
    let mut need_u = 0u32;
    let mut need_c = 0u32;
    for ch in cost.chars() {
        match ch {
            'R' => need_r += 1,
            'G' => need_g += 1,
            'U' => need_u += 1,
            '1'..='9' => need_c += ch.to_digit(10).unwrap(),
            _ => {}
        }
    }
    let bf = state.players[player_idx].battlefield.clone();
    for &oid in &bf {
        let o = state.objects.get_mut(&oid).unwrap();
        if o.tapped {
            continue;
        }
        let cid = &o.card_id;
        if need_r > 0 && cid == "mountain" {
            o.tapped = true;
            need_r -= 1;
        } else if need_g > 0 && cid == "forest" {
            o.tapped = true;
            need_g -= 1;
        } else if need_u > 0 && cid == "island" {
            o.tapped = true;
            need_u -= 1;
        }
    }
    let mut need = need_c + need_r + need_g + need_u;
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
        if matches!(o.card_id.as_str(), "mountain" | "forest" | "island") {
            o.tapped = true;
            need -= 1;
        }
    }
    if need > 0 {
        return Err(EngineError::Illegal("cannot pay mana"));
    }
    Ok(())
}

