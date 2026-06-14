use std::collections::{BTreeMap, HashMap, VecDeque};
use tricerules_cards::primitives::{ContinuousEffectKind, CounterKind, EffectDuration};

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
    /// CR 510.4 first combat damage step. Only entered when an attacker or blocker has first
    /// strike or double strike at the moment the combat damage step would begin. Both passes
    /// of combat damage live in their own step; the engine never lingers here if no first/double
    /// strike creature is involved.
    FirstStrikeDamage,
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
            DeclareAttackers | DeclareBlockers | FirstStrikeDamage | CombatDamage => None,
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
    Exile,
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
    /// True if this permanent has received any amount of damage from a source with deathtouch
    /// this turn (CR 702.2b / CR 704.5h). Cleared during the cleanup step alongside `damage`.
    pub deathtouch_damage: bool,
    /// Counters on this permanent (CR 122). `BTreeMap` for deterministic iteration/serialization.
    /// +1/+1 and -1/-1 counters here feed CR 613.4 layer 7d in the P/T computation and annihilate
    /// in pairs as a state-based action (CR 122.3). Unlike continuous effects, counters persist
    /// across cleanup — they are not until-end-of-turn effects.
    pub counters: BTreeMap<CounterKind, u32>,
}

impl GameObject {
    pub fn is_creature(&self, registry: &tricerules_cards::CardRegistry) -> bool {
        registry
            .get(&self.card_id)
            .map(|c| c.is_creature)
            .unwrap_or(false)
    }

    /// True if this object is a token (CR 111): created by an effect, not backed by a deck card.
    /// Tokens cease to exist as a state-based action once they leave the battlefield (CR 111.7).
    pub fn is_token(&self, registry: &tricerules_cards::CardRegistry) -> bool {
        registry.is_token(&self.card_id)
    }

    /// Number of counters of `kind` currently on this permanent (0 if none).
    pub fn counter_count(&self, kind: CounterKind) -> u32 {
        self.counters.get(&kind).copied().unwrap_or(0)
    }

    /// Set the number of `kind` counters, dropping the map entry when `n` is 0 so an emptied
    /// counter kind never lingers (keeps the map minimal and iteration deterministic).
    pub fn set_counter(&mut self, kind: CounterKind, n: u32) {
        if n == 0 {
            self.counters.remove(&kind);
        } else {
            self.counters.insert(kind, n);
        }
    }

    /// Net power/toughness delta from +1/+1 and -1/-1 counters (CR 613.4 layer 7d). Each pair of
    /// counters annihilates as an SBA (CR 122.3), so on a settled board only one kind remains,
    /// but this stays correct even before that SBA has run.
    pub fn counter_pt_delta(&self) -> i32 {
        self.counter_count(CounterKind::PlusOnePlusOne) as i32
            - self.counter_count(CounterKind::MinusOneMinusOne) as i32
    }

    /// Returns true if this permanent's card definition includes the given keyword ability.
    pub fn has_keyword(
        &self,
        registry: &tricerules_cards::CardRegistry,
        kw: tricerules_cards::Keyword,
    ) -> bool {
        registry
            .get(&self.card_id)
            .map(|c| c.keywords.contains(&kw))
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
    pub exile: Vec<ObjectId>,
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
            exile: Vec::new(),
            mana_pool: ManaPool::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ManaPool {
    pub white: u32,
    pub blue: u32,
    pub black: u32,
    pub red: u32,
    pub green: u32,
    pub colorless: u32,
}

impl ManaPool {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// A triggered ability that has fired but is waiting for the controller to choose a target
/// before being placed on the stack (CR 603.3d). Only one pending trigger at a time.
#[derive(Debug, Clone)]
pub struct PendingTrigger {
    pub source_permanent_id: ObjectId,
    pub ability_index: usize,
    pub ability_text: String,
    pub card_id: String,
    pub controller: PlayerId,
}

#[derive(Debug, Clone)]
pub struct StackItem {
    pub id: ObjectId,
    pub controller: PlayerId,
    pub card_id: String,
    pub targets: Vec<ObjectId>,
    /// `None` = spell; `Some(text)` = activated or triggered ability annotation shown on stack card.
    pub ability_text: Option<String>,
    /// For activated/triggered abilities: the permanent that sourced this ability (stays in its zone).
    pub source_permanent_id: Option<ObjectId>,
    /// Index into the card's `activated_abilities` or `triggered_abilities` list. `None` for spells.
    pub ability_index: Option<usize>,
    /// `true` = this is a triggered ability; `false` = activated ability or spell.
    pub is_triggered: bool,
    /// CR 709/712/715: which face of a multi-face card was cast (the split half / MDFC face /
    /// adventure half on the stack). `0` for single-face cards and abilities; drives which face's
    /// effects, mana value, and permanence are used when this spell resolves.
    pub face_index: usize,
    /// CR 107.3b: the value chosen for `{X}` as this spell was cast. `0` for spells without an
    /// `{X}` pip (and for abilities). On the stack the spell's mana value is `fixed_mv + chosen_x`;
    /// at resolution this feeds [`Amount::X`](tricerules_cards::Amount) effect amounts.
    pub chosen_x: u32,
}

/// Pre-game: choose first player, then London-style mulligans (redraw to 7, then put N on bottom).
#[derive(Debug, Clone)]
pub struct OpeningSequence {
    /// Seat id chosen by RNG to pick who goes first.
    pub chooser: PlayerId,
    /// Set once the chooser commits; that player takes the first turn.
    pub starting_player: Option<PlayerId>,
    /// Who must keep/mulligan or bottom cards next.
    pub mulligan_actor: Option<PlayerId>,
    /// During bottom step: (player, cards still to place on bottom).
    pub bottom: Option<(PlayerId, u32)>,
    /// Mulligans already taken this opening (indexed by `players` vec index).
    pub mulligans_taken: [u32; 2],
    /// Opening fully finished for each seat (indexed by `players` vec index).
    pub resolved: [bool; 2],
}

/// What set of permanents a continuous effect applies to (CR 613).
/// Using a scope enum (rather than a bare ObjectId) means anthem-style effects
/// ("all creatures get +1/+1") work correctly for permanents that enter after the effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AffectedScope {
    Single(ObjectId),
    AllCreatures,
    // Future: CreaturesControlledBy(PlayerId), CreaturesWithPower(u32), …
}

/// A single active continuous effect (CR 611/613).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousEffect {
    /// Spell/ability that created this (for display and future targeted removal).
    pub source_id: Option<ObjectId>,
    pub affected: AffectedScope,
    pub kind: ContinuousEffectKind,
    pub duration: EffectDuration,
    /// `command_index` at creation; used for layer sublayer timestamp ordering (CR 613.7).
    pub timestamp: u64,
}

impl ContinuousEffect {
    /// True if this effect applies to the permanent with id `oid`.
    pub fn affects(&self, oid: ObjectId) -> bool {
        match &self.affected {
            AffectedScope::Single(id) => *id == oid,
            AffectedScope::AllCreatures => true,
        }
    }
}

/// During combat, after attack/block declarations.
#[derive(Debug, Clone)]
pub struct CombatState {
    pub attacking: Vec<ObjectId>,
    /// Maps each attacker to all creatures blocking it (may be multiple — CR 509.1h).
    pub blockers: HashMap<ObjectId, Vec<ObjectId>>,
    /// Explicit combat damage from each multiply-blocked (or trample) attacker to its blockers;
    /// populated when the active player submits `AssignCombatDamage` for that attacker.
    pub damage_assignments: HashMap<ObjectId, Vec<(ObjectId, u32)>>,
    /// CR 702.19: for trample attackers, the excess damage assigned to the defending player
    /// (above the lethal damage dealt to all blockers). Keyed by attacker ObjectId.
    pub trample_player_damage: HashMap<ObjectId, u32>,
    /// True when blockers_declared and at least one attacker has 2+ blockers (or has trample
    /// with 1+ blockers) without a full `damage_assignments` entry; active player must assign.
    pub damage_assignment_needed: bool,
    /// True once active player has finalized attackers for this combat.
    pub attackers_declared: bool,
    /// True after the defending player has finalized blockers for this combat.
    pub blockers_declared: bool,
    /// True only after both players have passed priority in declare blockers while assignment
    /// is still required — then the active player may submit `AssignCombatDamage`.
    pub assign_combat_damage_phase: bool,
    /// Snapshot of attackers that participated in the first-strike damage step (CR 510.4).
    /// Captured immediately before first-strike damage resolves; empty if no first-strike
    /// step occurred (no attacker or blocker had FirstStrike/DoubleStrike).
    pub first_strike_attackers: Vec<ObjectId>,
    /// Snapshot of per-attacker blockers that participated in the first-strike damage step.
    /// Mirrors the layout of `blockers`. Used during the regular step to exclude creatures that
    /// already dealt damage (CR 510.4) unless they have DoubleStrike.
    pub first_strike_blockers: HashMap<ObjectId, Vec<ObjectId>>,
    /// True once the first-strike damage step has resolved for the current combat. Stays true
    /// for the rest of combat; the engine uses it to gate the regular-step participation rule
    /// and the `first_strike_step_pending` per-player-view flag.
    pub first_strike_damage_done: bool,
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
    /// CR 514.1: player who must discard next during cleanup, if any.
    pub cleanup_discard_player: Option<PlayerId>,
    /// Pre-game flow; `None` once the duel has started (upkeep of turn 1).
    pub opening: Option<OpeningSequence>,
    /// Seat index of the player who takes the first turn (CR 103.8: only they skip their first draw step).
    pub starting_player_idx: usize,
    /// Triggered abilities that have fired and are awaiting target selection before going on the
    /// stack (CR 603.3d). Queue supports simultaneous triggers (CR 603.3b APNAP ordering);
    /// processed front-to-back, one target choice at a time.
    pub pending_triggers: VecDeque<PendingTrigger>,
    /// Active continuous effects (CR 611/613). Effects are pushed here at resolution and drained
    /// at cleanup or when their source leaves the battlefield. P/T and other characteristics are
    /// recomputed from base + this list on demand — `GameObject.power/toughness` always hold the
    /// printed base value and are never mutated by effects.
    pub continuous_effects: Vec<ContinuousEffect>,
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
