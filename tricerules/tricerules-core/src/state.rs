use std::collections::{HashMap, VecDeque};

pub type PlayerId = i32;
pub type ObjectId = u32;

/// Turn structure for vanilla (no first-strike or trample substeps).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TurnStep {
    Untap,
    Upkeep,
    Draw,
    Main1,
    BeginCombat,
    DeclareAttackers,
    DeclareBlockers,
    CombatDamage,
    EndCombat,
    Main2,
    EndStep,
    Cleanup,
}

impl TurnStep {
    /// Next step after a full "pass priority with empty stack" in this step, or `None` if
    /// `pass_priority` must not advance (e.g. declare substeps are handled by explicit commands).
    pub fn next_after_all_pass(self) -> Option<TurnStep> {
        use TurnStep::*;
        match self {
            Untap => None, // auto-advances, should not reach pass
            Upkeep | Draw => Some(Main1),
            Main1 => Some(BeginCombat),
            BeginCombat => None, // moves to declare substeps on pass, see engine
            DeclareAttackers | DeclareBlockers | CombatDamage => None,
            EndCombat => Some(Main2),
            Main2 => Some(EndStep),
            EndStep => Some(Cleanup),
            Cleanup => None, // new turn: handled in engine
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Stack,
}

#[derive(Debug, Clone)]
pub struct GameObject {
    pub id: ObjectId,
    pub owner: PlayerId,
    pub card_id: String,
    pub zone: Zone,
    pub tapped: bool,
    pub summoning_sick: bool,
    pub power: Option<u32>,
    pub toughness: Option<u32>,
    pub damage: u32,
    /// Counters: used for SBA +0/+0 annihilation with -1 in future
    #[allow(dead_code)]
    pub plus_one_plus_one: u32,
    #[allow(dead_code)]
    pub minus_one_minus_one: u32,
}

impl GameObject {
    pub fn is_creature(&self, registry: &tricerules_cards::CardRegistry) -> bool {
        registry
            .get(&self.card_id)
            .map(|c| c.is_creature)
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub struct PlayerState {
    pub id: PlayerId,
    pub life: i32,
    /// Out of game: lost
    pub has_lost: bool,
    pub library: VecDeque<ObjectId>,
    pub hand: Vec<ObjectId>,
    pub battlefield: Vec<ObjectId>,
    pub graveyard: Vec<ObjectId>,
    pub mana_pool: ManaPool,
}

impl PlayerState {
    pub fn new(id: PlayerId, life: i32) -> Self {
        PlayerState {
            id,
            life,
            has_lost: false,
            library: VecDeque::new(),
            hand: Vec::new(),
            battlefield: Vec::new(),
            graveyard: Vec::new(),
            mana_pool: ManaPool::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ManaPool {
    pub red: u32,
    pub green: u32,
    pub blue: u32,
    pub colorless: u32,
}

impl ManaPool {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone)]
pub struct StackItem {
    pub id: ObjectId,
    pub controller: PlayerId,
    pub card_id: String,
    pub targets: Vec<ObjectId>,
}

/// During combat, after attack/block declarations.
#[derive(Debug, Clone)]
pub struct CombatState {
    pub attacking: Vec<ObjectId>,
    /// Each attacker at most one blocker
    pub blocker: HashMap<ObjectId, ObjectId>,
    /// True once active player has finalized attackers for this combat.
    pub attackers_declared: bool,
    /// True after the defending player has finalized blockers for this combat.
    pub blockers_declared: bool,
}

#[derive(Debug)]
pub struct GameState {
    pub seed: u64,
    pub players: Vec<PlayerState>,
    pub objects: HashMap<ObjectId, GameObject>,
    pub stack: Vec<StackItem>,
    /// Index into players for who holds priority
    pub priority_idx: usize,
    pub active_player_idx: usize,
    pub turn_step: TurnStep,
    pub turn: u32,
    pub next_object_id: ObjectId,
    pub command_index: u64,
    /// Consecutive priority passes; reset when a spell/ability is added to stack
    pub passes_since_stack_change: u32,
    /// At most one land from hand per full turn
    pub land_dropped_this_turn: bool,
    /// Active combat, if in declare/damage
    pub combat: Option<CombatState>,
    /// If set, game is over; winning player
    pub winner: Option<PlayerId>,
}

impl GameState {
    pub fn player_idx(&self, pid: PlayerId) -> Option<usize> {
        self.players.iter().position(|p| p.id == pid)
    }

    pub fn active_player_id(&self) -> PlayerId {
        self.players[self.active_player_idx].id
    }

    pub fn priority_player_id(&self) -> PlayerId {
        self.players[self.priority_idx].id
    }

    /// The defending player in 1v1 (opponent of active) — for multi-player use first NAP; M2: two players
    pub fn defending_player_id_1v1(&self) -> Option<PlayerId> {
        if self.players.len() != 2 {
            return None;
        }
        let a = self.active_player_idx;
        let d = 1 - a;
        Some(self.players[d].id)
    }
}
