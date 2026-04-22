use std::collections::{HashMap, VecDeque};

pub type PlayerId = i32;
pub type ObjectId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    Main1,
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
    pub library: VecDeque<ObjectId>,
    pub hand: Vec<ObjectId>,
    pub battlefield: Vec<ObjectId>,
    pub graveyard: Vec<ObjectId>,
}

impl PlayerState {
    pub fn new(id: PlayerId, life: i32) -> Self {
        PlayerState {
            id,
            life,
            library: VecDeque::new(),
            hand: Vec::new(),
            battlefield: Vec::new(),
            graveyard: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StackItem {
    pub id: ObjectId,
    pub controller: PlayerId,
    pub card_id: String,
    pub targets: Vec<ObjectId>,
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
    pub phase: GamePhase,
    pub turn: u32,
    pub next_object_id: ObjectId,
    pub command_index: u64,
    /// Consecutive priority passes; reset when a spell/ability is added to stack
    pub passes_since_stack_change: u32,
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
}
