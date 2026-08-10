use std::collections::{BTreeMap, HashMap, VecDeque};
use tricerules_cards::primitives::{
    Color, ContinuousEffectKind, CounterKind, DamagePreventionAdditionalEffect, EffectDuration,
    Keyword, ManaAmount, SearchDestination,
};
use tricerules_proto::ruled::v1::{ChoiceKind, RuledEvent, TokenCreated};

pub type PlayerId = i32;
pub type ObjectId = u32;

/// CR 715.3d: the player who resolved this card as an Adventure may cast one specific permanent
/// face for as long as this exact object remains exiled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdventureCastPermission {
    pub player_id: PlayerId,
    pub face_index: usize,
}

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
    /// CR 108.3: the player who started the game with this card in their deck. **Never changes**
    /// for the life of the object. Decides which zone the card goes to when it leaves the
    /// battlefield (CR 400.3), which player's hand/library/graveyard/exile it belongs to, and
    /// therefore what the hidden-info redaction is allowed to show whom.
    pub owner: PlayerId,
    /// CR 110.2: the player who currently controls this permanent. Equal to [`Self::owner`]
    /// except while a permanent is on the battlefield under someone else's control (reanimation
    /// today; Mind Control / Threaten when layer-2 *continuous* control lands). Decides untap and
    /// summoning sickness (CR 302.6/502), attack and block legality, ability control and APNAP
    /// order (CR 603.3), and anthem / "you control" scoping.
    ///
    /// This is the CR 613 layer-2 **base value**: `GameEngine::characteristics()` reads it and
    /// then applies control-changing continuous effects on top (none exist yet).
    /// Reset to `owner` whenever the object leaves the battlefield (CR 400.7 — new object).
    ///
    /// **Invariant**, asserted at the end of `apply_sbas`:
    /// ```text
    /// oid ∈ players[i].battlefield  ⟺  objects[oid].zone == Battlefield
    ///                               &&  objects[oid].controller == players[i].id
    /// ```
    /// i.e. `PlayerState::battlefield` is the *control* list, while `hand`/`library`/`graveyard`/
    /// `exile` are *owner* lists.
    pub controller: PlayerId,
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
    /// CR 303.4 / CR 301.5 / 702.6: for Aura or Equipment permanents, the `ObjectId` of the
    /// permanent this card is attached to. `None` for non-aura, non-equipment permanents or
    /// before attachment is established. Cleared on zone change (auras die; equipment falls off).
    pub attached_to: Option<ObjectId>,
    /// CR 701.15: number of regeneration shields on this permanent. Each shield is a replacement
    /// effect: the next time this permanent would be destroyed, instead tap it, remove it from
    /// combat, and clear all damage from it. Shields expire at the cleanup step (like damage).
    pub regeneration_shields: u32,
    /// CR 508.1d: this creature must be declared as an attacker whenever it is a legal attacker
    /// ("attacks each combat if able"). Set from card data at object creation; may be overridden
    /// by continuous effects. Cards: Crazed Goblin, Goblin Brigand.
    pub must_attack_if_able: bool,
    /// CR 509.1c: this creature must be declared as a blocker whenever it could legally block
    /// ("blocks each combat if able"). Set from card data; may be overridden by continuous effects.
    pub must_block_if_able: bool,
    /// CR 712.4: which face is currently showing on the battlefield. 0 = front (default).
    /// Always 0 for Normal single-face cards; set from `StackItem.face_index` when a multi-face
    /// permanent enters the battlefield (MDFC, Transform, Flip). Engine reads characteristics
    /// through this so the active face's types/keywords/P/T are used everywhere, not the front.
    pub face_up_index: usize,
    /// Present only while this exact object remains exiled after its Adventure face resolved.
    pub adventure_cast_permission: Option<AdventureCastPermission>,
}

/// Runtime scope of one damage-prevention effect. Player ids use the engine's existing widened
/// `ObjectId` convention, so `Recipient` covers both players and permanents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamagePreventionScope {
    Recipient(ObjectId),
    Combat,
    OtherCreaturesYouControl {
        source_id: ObjectId,
        controller: PlayerId,
    },
}

/// How much damage one active prevention effect can prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamagePreventionAmount {
    All,
    FixedPerEvent(u32),
    Remaining(u32),
}

/// One independently identifiable prevention effect. IDs are opaque ordering-choice values;
/// source identity is kept separate because several effects may come from one object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveDamagePrevention {
    pub id: u32,
    pub source_id: Option<ObjectId>,
    pub source_label: String,
    pub scope: DamagePreventionScope,
    pub amount: DamagePreventionAmount,
    pub duration: EffectDuration,
    pub additional_effect: Option<DamagePreventionAdditionalEffect>,
}

/// A prohibition is not a prevention effect (CR 615.12). Keeping it in a separate collection
/// ensures prevention applications can still run without consuming finite effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamagePreventionProhibition {
    pub source_id: Option<ObjectId>,
}

impl GameObject {
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

    /// Human-readable annotation of the counters on this permanent for client display,
    /// one line per counter kind (e.g. `"1 +1/+1 counter(s)"`). Empty string when the
    /// permanent has no counters. Iteration order is deterministic (`BTreeMap`).
    pub fn counter_annotation(&self) -> String {
        self.counters
            .iter()
            .filter(|&(_, &n)| n > 0)
            .map(|(kind, &n)| format!("{} {} counter(s)", n, kind.label()))
            .collect::<Vec<_>>()
            .join("\n")
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

/// A triggered ability that has fired but has not been put on the stack yet — the unit staged
/// between trigger collection and [`StackItem`].
///
/// `object_id` is reserved at collection time rather than at stack push. That reservation is what
/// gives an off-stack trigger a stable handle: it is what `TriggerOrderRequired` publishes, what
/// `SubmitTriggerOrder` echoes back, and it becomes this trigger's `StackItem::id` /
/// `StackPushed.object_id` once it is finally placed. Allocation follows the deterministic APNAP
/// collection order, so replays are unaffected.
#[derive(Debug, Clone)]
pub struct StagedTrigger {
    pub object_id: ObjectId,
    pub source_permanent_id: ObjectId,
    pub source_face_index: usize,
    pub source_zone_change: u64,
    pub source_face_change: u64,
    pub card_id: String,
    /// Registry display name captured at collection, so a source that has already left the game
    /// still renders in the ordering prompt (a Blood Artist that died in the same wipe).
    pub card_name: String,
    /// CR 603.3d: the ability's controller — the controller of its source permanent.
    pub controller: PlayerId,
    pub ability_index: usize,
    pub ability_text: String,
    /// The event's beneficiary when the trigger names a player other than its controller
    /// ("**that player** draws a card"); `None` means the controller.
    pub trigger_player: Option<PlayerId>,
    /// The distinct permanent whose becoming a target caused this trigger. Kept separate from
    /// `targets`: this object was observed by the trigger and was not chosen as its CR 115 target.
    pub trigger_object: Option<ObjectId>,
    /// CR 603.5: an optional triggered ability may be declined before it is put on the stack.
    pub may: bool,
}

/// The triggered abilities from *one* simultaneous event (CR 603.3b), in APNAP order and therefore
/// contiguous per controller. Drained front-to-back by `flush_staged_triggers`, which is what turns
/// "contiguous per controller" into "one ordering prompt per player".
#[derive(Debug, Clone)]
pub struct StagedTriggerGroup {
    pub triggers: Vec<StagedTrigger>,
}

/// CR 603.3b: one player controls two or more triggers from the same event and chooses the order
/// they are put on the stack. Mirrors [`PendingResolution`] — while present it blocks every command
/// but the answer, and the answer is itself a logged command, so replay determinism holds.
///
/// Drained one trigger at a time rather than answered with a permutation, because CR 603.3d picks
/// each ability's targets *as it is put on the stack*: the player names the next trigger, the engine
/// places it and asks for its target, and only then does the next choice come up. `candidates` is
/// therefore always the *remaining unplaced* triggers of the block, shrinking with each answer.
#[derive(Debug, Clone)]
pub struct PendingTriggerOrder {
    pub deciding_player: PlayerId,
    /// Still-unplaced triggers of this player's block, in engine (APNAP-stable) order.
    pub candidates: Vec<StagedTrigger>,
    /// Whether the client has already been told about the current `candidates` set. The drain is
    /// re-entered several times per command (once by the handler, once by `dispatch_command`'s
    /// tail), and without this the same prompt would be emitted twice in one batch.
    /// Cleared whenever `candidates` changes.
    pub prompt_emitted: bool,
}

/// Why the engine is refusing everything but one specific answer. Ordered by precedence in
/// [`GameState::blocking_choice`]; each variant names the single command that clears it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockingChoice {
    /// A tier-3 custom resolution is parked mid-resolution (CR 608).
    Resolution,
    /// Simultaneous triggers are staged awaiting their controller's ordering (CR 603.3b).
    TriggerOrder,
    /// A trigger is parked awaiting its target before it reaches the stack (CR 603.3d).
    TriggerTarget,
}

/// A triggered ability that has fired but is waiting for the controller to choose a target
/// before being placed on the stack (CR 603.3d). Only one pending trigger at a time.
#[derive(Debug, Clone)]
pub struct PendingTrigger {
    /// The id reserved for this trigger at collection; becomes its `StackItem::id`. See
    /// [`StagedTrigger::object_id`].
    pub object_id: ObjectId,
    pub source_permanent_id: ObjectId,
    pub source_face_index: usize,
    pub source_zone_change: u64,
    pub source_face_change: u64,
    pub ability_index: usize,
    pub ability_text: String,
    pub card_id: String,
    pub controller: PlayerId,
    /// CR 603.5: an optional triggered ability may be declined before it is put on the stack.
    pub may: bool,
    /// Mirror of [`StackItem::trigger_player`], carried across target selection so a targeted
    /// draw-step-style trigger keeps its beneficiary when it finally reaches the stack.
    pub trigger_player: Option<PlayerId>,
    /// Mirror of [`StackItem::trigger_object`], carried across any CR 603.3d target choice.
    pub trigger_object: Option<ObjectId>,
}

/// A tier-3 custom resolution (CR 608) parked mid-way, waiting on a player choice. Mirrors
/// [`PendingTrigger`]: while present it blocks priority, and the deciding player's
/// `SubmitResolutionChoice` command drives it forward. Stores only primitives + the in-flight
/// [`StackItem`] (no `dyn CardEffect`) — the `CardEffect` is re-looked-up by `custom_key` so the
/// engine never has to box a trait object into state. See `engine`/`custom` for the flow.
#[derive(Debug, Clone)]
pub struct PendingResolution {
    /// The spell whose resolution is paused (its card already moved to graveyard/battlefield).
    pub item: StackItem,
    /// Key into the `custom` registry; re-resolves to the `CardEffect` each step.
    pub custom_key: String,
    /// Choices already answered (0 means `begin` produced the outstanding interrupt).
    pub step: u32,
    /// Object ids the effect carries between steps (e.g. Gifts' revealed set).
    pub scratch: Vec<ObjectId>,
    /// The player who must answer the outstanding interrupt (may be the opponent — Gifts Ungiven).
    pub deciding_player: PlayerId,
    /// The legal candidate object ids; the engine validates the submitted choice is a subset.
    pub candidates: Vec<ObjectId>,
    pub min: u32,
    pub max: u32,
    /// Whether the submitted order is significant (Brainstorm: top-of-library order).
    pub ordered: bool,
    /// Display-only prompt text (re-emitted on reconnect; no Oracle lookup).
    pub prompt: String,
    /// What the candidates are, for client presentation and relay redaction (see `ChoiceKind`).
    pub choice_kind: ChoiceKind,
    /// Mirror of [`ResolutionInterrupt::unique_names`]: the engine rejects submissions where two
    /// chosen object ids map to the same card name (Gifts Ungiven: "different names").
    pub unique_names: bool,
    /// For `__copy_targets` only: the object id of the spell being copied, so `StackPushed` can
    /// carry `copy_source_object_id` for the client's printing-inheritance logic.
    pub copy_source_object_id: ObjectId,
    /// For `__search_library` only: where the found card goes (Hand or TopOfLibrary).
    pub search_destination: SearchDestination,
    /// For `__search_library` only: whether to shuffle the library after the search.
    pub search_shuffle: bool,
    /// For `__search_library` only: whether to publicly log the found card's name (reveal).
    pub search_reveal: bool,
    /// Index into the resolving spell's rebuilt effect list of the *next* primitive to run once
    /// this choice is answered (CR 608.2: a suspended effect must not swallow the rest of the
    /// list). Stamped by `run_effect_list` on whatever the suspending handler parked, so handlers
    /// themselves always write `None`. Stays `None` for parks that own no primitive effect list:
    /// tier-3 [`crate::custom::CardEffect`]s, `__copy_targets`, and the `__legend_sba` SBA.
    pub resume_effect_index: Option<u32>,
}

/// CR 616.1 priority groups. The current card set exercises `Other`; the complete ordering
/// vocabulary keeps entry-copy/control work from inventing a second chooser later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)] // The non-Other classes are the CR 616 slots for tracked entry-copy/control work.
pub(crate) enum ReplacementPriority {
    SelfReplacement,
    EntryControl,
    EntryCopy,
    EntryBackFace,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntryReplacementEffectId {
    Intrinsic {
        object_id: ObjectId,
        ability_index: usize,
    },
    Battlefield {
        source_id: ObjectId,
        source_generation: u64,
        ability_index: usize,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct BattlefieldEntryEvent {
    pub object_id: ObjectId,
    /// CR 616.1 decider: current controller, or owner when the object has no controller.
    pub deciding_player: PlayerId,
    pub destination_controller: PlayerId,
    pub face_index: usize,
    pub tapped: bool,
    pub applied_effects: Vec<EntryReplacementEffectId>,
}

#[derive(Debug, Clone)]
pub(crate) struct EntryReplacementApplication {
    pub application_id: u32,
    pub effect_id: EntryReplacementEffectId,
}

#[derive(Debug, Clone)]
pub(crate) struct TokenBattlefieldEntry {
    pub event: BattlefieldEntryEvent,
    pub created: TokenCreated,
}

#[derive(Debug, Clone)]
pub(crate) enum BattlefieldEntryCompletion {
    LandPlay {
        player: PlayerId,
        land_name: String,
    },
    PermanentSpell {
        attached_to: Option<ObjectId>,
    },
    ResolutionEffect {
        owner: PlayerId,
        spell_label: String,
        object_label: String,
    },
    TokenBatch {
        current_created: TokenCreated,
        ready: Vec<TokenBattlefieldEntry>,
        remaining: Vec<TokenBattlefieldEntry>,
        logs: Vec<String>,
    },
    DevPlacement {
        target: PlayerId,
        ready: bool,
        name: String,
        verb: String,
        deferred_events: Vec<RuledEvent>,
        announce_move: bool,
    },
}

/// A proposed battlefield entry parked before any zone mutation. The parallel
/// `pending_resolution` owns the public prompt and command validation.
#[derive(Debug, Clone)]
pub(crate) struct PendingBattlefieldEntry {
    pub event: BattlefieldEntryEvent,
    pub applications: Vec<EntryReplacementApplication>,
    pub completion: BattlefieldEntryCompletion,
}

#[derive(Debug, Clone)]
pub struct ChosenSpellMode {
    pub mode_index: usize,
    pub targets: Vec<ObjectId>,
    pub target_damage: Vec<u32>,
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
    /// Generation of the source permanent when this ability was put on the stack (CR 400.7).
    /// A matching ObjectId after a leave-and-return is a different object and must not receive
    /// the old ability's self-bound effect.
    pub source_zone_change: u64,
    /// Face-change generation captured when an ability was put on the stack (CR 701.27f).
    pub source_face_change: u64,
    /// Index into the card's `activated_abilities` or `triggered_abilities` list. `None` for spells.
    pub ability_index: Option<usize>,
    /// `true` = this is a triggered ability; `false` = activated ability or spell.
    pub is_triggered: bool,
    /// CR 707.10: this stack item is a *copy* of a spell (Twincast/Fork), not a cast object. A
    /// copy has no backing [`GameObject`] in `objects` — like an ability it leaves no card behind,
    /// so resolution skips the zone move and the copy ceases to exist (CR 707.10d). It also never
    /// fires cast triggers or counts toward storm (it was put on the stack, not cast).
    pub is_copy: bool,
    /// CR 709/712/715: which face of a multi-face card was cast (the split half / MDFC face /
    /// adventure half on the stack). `0` for single-face cards and abilities; drives which face's
    /// effects, mana value, and permanence are used when this spell resolves.
    pub face_index: usize,
    /// CR 702.34: this spell was cast from a graveyard using flashback and is exiled instead of
    /// being put into its owner's graveyard when it leaves the stack.
    pub flashback: bool,
    /// CR 107.3b: the value chosen for `{X}` as this spell was cast. `0` for spells without an
    /// `{X}` pip (and for abilities). On the stack the spell's mana value is `fixed_mv + chosen_x`;
    /// at resolution this feeds [`Amount::X`](tricerules_cards::Amount) effect amounts.
    pub chosen_x: u32,
    /// Per-target damage amounts for `DamageTargets` (Fireball, Fire). Parallel to `targets`:
    /// `target_damage[i]` is the damage allocated to `targets[i]` by the casting player.
    /// Empty for all other effects (no overhead for non-Fireball spells).
    pub target_damage: Vec<u32>,
    /// Atomic modal choices in printed order. Empty for nonmodal spells and abilities.
    pub chosen_modes: Vec<ChosenSpellMode>,
    /// The player a triggered ability's effects act on when the trigger names someone other than
    /// its controller — "at the beginning of each player's draw step, **that player** draws an
    /// additional card" (Howling Mine, Kami of the Crescent Moon). `None` (spells, activated
    /// abilities, and every other trigger) means the effects act on [`Self::controller`].
    /// Controllership itself is unaffected: the Mine's controller still controls the ability and
    /// decides its stack ordering.
    pub trigger_player: Option<PlayerId>,
    /// The permanent whose becoming a target caused this triggered ability. This is event
    /// identity, not one of this stack item's own CR 115 targets, and therefore remains available
    /// even for a non-targeted observer trigger.
    pub trigger_object: Option<ObjectId>,
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
    /// Mulligans already taken this opening (indexed by `players` vec index, one entry per seat).
    pub mulligans_taken: Vec<u32>,
    /// Opening fully finished for each seat (indexed by `players` vec index, one entry per seat).
    pub resolved: Vec<bool>,
}

/// The next seat that has not finished its opening, scanning forward in seat order from `start_idx`
/// inclusive and wrapping. `None` once every seat is resolved.
///
/// The single source of opening-turn order (CR 103.4: each player in turn order decides whether to
/// mulligan). Both `opening.rs` call sites previously rolled their own — one as `1 - idx`, the other
/// as a two-element array — and neither generalizes past two seats. Same reason `apnap_rank` and
/// `battlefield_sources_apnap` exist: an ordering with two implementations is an ordering that
/// drifts.
pub fn next_unresolved_from(resolved: &[bool], start_idx: usize) -> Option<usize> {
    let seats = resolved.len();
    (0..seats)
        .map(|offset| (start_idx + offset) % seats)
        .find(|&idx| !resolved[idx])
}

/// What set of permanents a continuous effect applies to (CR 613).
/// Using a scope enum (rather than a bare ObjectId) means anthem-style effects
/// ("all creatures get +1/+1") work correctly for permanents that enter after the effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AffectedScope {
    Single(ObjectId),
    AllCreatures,
    /// Creatures matching an anthem/lord filter (CR 613) — resolved from an [`AnthemFilter`] when
    /// the source's static ability or one-shot pump fires. Evaluated dynamically each P/T query so
    /// creatures entering *after* the effect are still affected. `controller` `None` = any player's
    /// creatures; `Some(pid)` = only `pid`'s. `subtype`/`color` narrow by characteristics
    /// (Lord of Atlantis = Merfolk, Bad Moon = Black). `exclude` is the source for "other ...
    /// creatures" lords. Membership needs card characteristics, so it is evaluated in the engine
    /// (which holds the registry), not in [`ContinuousEffect::affects`].
    CreaturesMatching {
        controller: Option<PlayerId>,
        subtype: Option<String>,
        color: Option<Color>,
        exclude: Option<ObjectId>,
    },
    /// The permanent currently attached to the Aura or Equipment with `source_oid`. Resolved
    /// dynamically from the source object's `attached_to`, so re-equipping moves every modifier
    /// together without recreating effects.
    AttachedTo(ObjectId),
    /// Effect that targets a specific player rather than permanents (e.g. extra land plays,
    /// future hand-size modifiers). Not considered by `effect_affects` (permanent queries).
    Player(PlayerId),
    // Future: CreaturesWithPower(u32), …
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

/// A still-rewindable mana ability the priority player just activated (CR 605). Recorded only for
/// the classic, fully-reversible case — a pure `{T}` mana ability — so an undo is a clean untap +
/// pool removal with no mana/life/sacrifice cost to refund. The engine drops every entry the moment
/// the float becomes consequential (mana spent, spell/ability cast, priority passed, step change),
/// so a present entry is always safe to undo.
#[derive(Debug, Clone)]
pub struct UndoableManaAbility {
    /// The player who activated it (and the only one who may undo it).
    pub player: PlayerId,
    /// The permanent that was tapped to produce the mana.
    pub source: ObjectId,
    /// Mana added to `player`'s pool by this activation; removed verbatim on undo.
    pub produced: ManaAmount,
}

#[derive(Debug)]
pub struct GameState {
    pub seed: u64,
    pub players: Vec<PlayerState>,
    pub objects: HashMap<ObjectId, GameObject>,
    /// CR 608.2h / 113.7a last known information: the tap status each object had as it left the
    /// battlefield, recorded in `move_object_to_zone` just before CR 400.7 resets it. A spell or
    /// ability still on the stack that asks about a permanent which has since left uses this
    /// rather than the reset value — Howling Mine's intervening-"if" is the first consumer
    /// (bounced while untapped, the ability still resolves). Keyed by object, so a later LKI need
    /// (P/T of a creature that died mid-resolution) joins it here rather than snapshotting ad hoc.
    pub last_known_tapped: HashMap<ObjectId, bool>,
    /// Last-known tap status keyed by the source object's generation, so an older trigger is not
    /// confused by a later leave-and-return cycle using the same relay ObjectId.
    pub last_known_tapped_by_generation: HashMap<(ObjectId, u64), bool>,
    /// Last-known derived keywords keyed by source object and generation. Resolution uses this
    /// after an ability's source leaves or returns, so the new object cannot retroactively change
    /// whether damage from the old source had deathtouch (CR 702.2e).
    pub last_known_keywords_by_generation: HashMap<(ObjectId, u64), Vec<Keyword>>,
    /// Monotonic per-object generation incremented on every zone change. ObjectIds remain stable
    /// for relay compatibility, while this generation preserves CR 400.7 identity semantics for
    /// effects that resolve after a source leaves and returns.
    pub zone_change_generation: HashMap<ObjectId, u64>,
    /// Incremented whenever a battlefield permanent changes face/status in place.
    pub face_change_generation: HashMap<ObjectId, u64>,
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
    /// Number of lands played from hand this turn; compared against max (1 + extra land plays).
    pub lands_played_this_turn: u32,
    pub spells_cast_this_turn: u32,
    pub spells_cast_last_turn: u32,
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
    /// Triggered abilities collected from simultaneous events but not yet put on the stack. A queue
    /// of *groups* because several events can fire while one command is applied (a resolution and
    /// the SBA cascade it causes); CR 603.3b applies per event, so each group is ordered
    /// independently. Drained by `flush_staged_triggers` at the two points where the engine is
    /// between actions.
    pub staged_trigger_groups: VecDeque<StagedTriggerGroup>,
    /// The outstanding CR 603.3b ordering prompt, or `None`. At most one at a time; while set it
    /// blocks every command but `SubmitTriggerOrder`.
    pub pending_trigger_order: Option<PendingTriggerOrder>,
    /// A tier-3 custom resolution paused mid-way awaiting a player choice (CR 608), or `None`.
    /// At most one at a time; while set it blocks priority (like a [`PendingTrigger`]) until the
    /// deciding player submits their choice. See [`PendingResolution`] and the `custom` module.
    pub pending_resolution: Option<PendingResolution>,
    /// Damage or battlefield-entry processing parked behind a public CR 616 ordering choice. The
    /// parallel `pending_resolution` owns the generic prompt/continuation metadata.
    pub(crate) pending_replacement_event:
        Option<crate::engine::replacement::PendingReplacementEvent>,
    /// Active continuous effects (CR 611/613). Effects are pushed here at resolution and drained
    /// at cleanup or when their source leaves the battlefield. P/T and other characteristics are
    /// recomputed from base + this list on demand — `GameObject.power/toughness` always hold the
    /// printed base value and are never mutated by effects.
    pub continuous_effects: Vec<ContinuousEffect>,
    /// Active CR 615 prevention effects. Healing Salve and Fog are both represented here so every
    /// producer enters one event pipeline and finite effects have stable opaque identities.
    pub damage_prevention_effects: Vec<ActiveDamagePrevention>,
    /// Active CR 615.12 prohibitions, deliberately separate from prevention effects.
    pub damage_prevention_prohibitions: Vec<DamagePreventionProhibition>,
    pub next_damage_prevention_effect_id: u32,
    /// Opaque ids exposed by replacement-order prompts. Separate from effect identities so stale
    /// submissions cannot name a newly-applicable effect after recomputation.
    pub next_replacement_application_id: u32,
    /// Mana abilities the priority player has activated this priority window that are still
    /// inconsequential and may be rewound by `UndoManaAbility` (CR 605 float courtesy). Newest
    /// last. Cleared by any consequential action (see [`UndoableManaAbility`]); empty whenever no
    /// fresh float is undoable.
    pub undoable_mana_abilities: Vec<UndoableManaAbility>,
    /// Permanents that *became untapped* (CR 701.20) during the command currently being applied.
    /// An edge log, not rules state: [`crate::engine::set_tapped`] appends here only when it
    /// actually flips a permanent from tapped to untapped, and `apply_command` drains it into the
    /// response batch's `PermanentsUntapped` event. Nothing in the rules ever reads it, so it is
    /// deliberately excluded from snapshots — a reconnecting client learns tap state from the
    /// zone view instead.
    pub untapped_this_command: Vec<ObjectId>,
}

impl GameState {
    pub fn add_damage_prevention_shield(&mut self, recipient: ObjectId, amount: u32) {
        let id = self.next_damage_prevention_effect_id;
        self.next_damage_prevention_effect_id = id.saturating_add(1);
        self.damage_prevention_effects.push(ActiveDamagePrevention {
            id,
            source_id: None,
            source_label: "Prevention shield".to_string(),
            scope: DamagePreventionScope::Recipient(recipient),
            amount: DamagePreventionAmount::Remaining(amount),
            duration: EffectDuration::UntilEndOfTurn,
            additional_effect: None,
        });
    }

    pub fn remaining_damage_prevention(&self, recipient: ObjectId) -> u32 {
        self.damage_prevention_effects
            .iter()
            .filter_map(|effect| match (effect.scope, effect.amount) {
                (
                    DamagePreventionScope::Recipient(target),
                    DamagePreventionAmount::Remaining(amount),
                ) if target == recipient => Some(amount),
                _ => None,
            })
            .sum()
    }

    pub fn player_idx(&self, pid: PlayerId) -> Option<usize> {
        self.players.iter().position(|p| p.id == pid)
    }

    pub fn active_player_id(&self) -> PlayerId {
        self.players[self.active_player_idx].id
    }

    pub fn priority_player_id(&self) -> PlayerId {
        self.players[self.priority_idx].id
    }

    /// The outstanding player decision the engine is waiting on, if any — the single source of
    /// "what is the game blocked on".
    ///
    /// All three block the same way and for the same reason: each is a choice the rules require
    /// *before* any player receives priority, so letting another command through would build a
    /// wrong or half-finished stack. `dispatch_command` rejects everything but the matching answer
    /// (and `Concede`, CR 104.3a).
    ///
    /// Precedence matters, and all three cases are live at once during a multi-trigger ordering:
    /// a parked resolution outranks everything, because the spell that produced the triggers is
    /// still resolving (CR 603.3: they wait for the next time a player would receive priority).
    /// A parked *target* then outranks the ordering prompt: the trigger just chosen is mid-placement
    /// and CR 603.3d resolves its target before the next one is picked, so `pending_trigger_order`
    /// legitimately still holds the remaining candidates while a target is outstanding.
    pub fn blocking_choice(&self) -> Option<BlockingChoice> {
        if self.pending_resolution.is_some() {
            return Some(BlockingChoice::Resolution);
        }
        if !self.pending_triggers.is_empty() {
            return Some(BlockingChoice::TriggerTarget);
        }
        if self.pending_trigger_order.is_some() {
            return Some(BlockingChoice::TriggerOrder);
        }
        None
    }

    /// Seat order rotated to start at the active player (CR 101.4 APNAP). Lower rank goes on the
    /// stack first (CR 603.3b).
    ///
    /// The single source of APNAP ranking for triggers. Two call sites previously rolled their own
    /// `(controller != active) as u8` key, which is a *boolean*: with three or more seats it does
    /// not separate the nonactive players from each other, so their triggers interleaved.
    pub fn apnap_rank(&self, player: PlayerId) -> usize {
        let seats = self.players.len();
        let Some(idx) = self.player_idx(player) else {
            return seats;
        };
        (idx + seats - self.active_player_idx) % seats
    }

    /// Every defending player this turn (CR 506.2): each nonactive player still in the game, in
    /// APNAP order (CR 101.4). Exactly one element in 1v1.
    pub fn defending_player_ids(&self) -> Vec<PlayerId> {
        defending_player_ids_of(&self.players, self.active_player_idx)
    }

    /// Whether `player` is a defending player this turn — the "is this your call?" question, asked
    /// by every command guard that gates a blocking decision. Seat-count generic.
    pub fn is_defending_player(&self, player: PlayerId) -> bool {
        self.defending_player_ids().contains(&player)
    }

    /// The defending player, but only when there is exactly one.
    ///
    /// **This is the engine's single remaining hard 2-player assumption.** Combat commands carry no
    /// per-attacker defender (`DeclareAttackers` is a bare list of creature ids), so any code that
    /// needs to name *the* defender cannot work with more than one — it calls this and fails closed.
    /// Widening the engine past two seats means giving each of these a real defender instead:
    /// `combat.rs` `defending_player_has_eligible_blockers`, `required_blocker_ids`, `set_blockers`,
    /// and combat damage assignment. Nothing else in the engine depends on the seat count; see
    /// `SUPPORTED_PLAYER_COUNT` in `engine/mod.rs`.
    pub fn sole_defending_player_id(&self) -> Option<PlayerId> {
        let defenders = self.defending_player_ids();
        match defenders.len() {
            1 => Some(defenders[0]),
            _ => None,
        }
    }
}

/// Seat-generic body of [`GameState::defending_player_ids`], split out so multi-seat behaviour is
/// unit-testable without building a whole [`GameState`].
fn defending_player_ids_of(players: &[PlayerState], active_player_idx: usize) -> Vec<PlayerId> {
    let seats = players.len();
    (1..seats)
        .map(|offset| &players[(active_player_idx + offset) % seats])
        .filter(|p| !p.has_lost)
        .map(|p| p.id)
        .collect()
}

/// Seat-order unit tests.
///
/// These deliberately exercise 3 and 4 seats, which `GameEngine::new` refuses to build (see
/// `SUPPORTED_PLAYER_COUNT`). Both helpers are seat-generic by construction, and this is the only
/// place that can prove it while the constructor gate stands — which is the point: the arithmetic
/// they replaced (`1 - idx`, `[start, 1 - start]`, `resolved[0] && resolved[1]`) could not be
/// tested at all.
#[cfg(test)]
mod seat_order_tests {
    use super::*;

    fn seats(ids: &[PlayerId]) -> Vec<PlayerState> {
        ids.iter().map(|&id| PlayerState::new(id, 20)).collect()
    }

    #[test]
    fn next_unresolved_scans_forward_and_wraps() {
        assert_eq!(next_unresolved_from(&[false, false], 0), Some(0));
        assert_eq!(next_unresolved_from(&[false, false], 1), Some(1));
        // Wraps past the end back to seat 0.
        assert_eq!(next_unresolved_from(&[false, true, true], 1), Some(0));
        // Skips resolved seats in between.
        assert_eq!(next_unresolved_from(&[true, true, false, true], 1), Some(2));
        // Starting seat counts itself first.
        assert_eq!(next_unresolved_from(&[true, false, false], 1), Some(1));
    }

    #[test]
    fn next_unresolved_is_none_once_every_seat_is_done() {
        assert_eq!(next_unresolved_from(&[true, true], 0), None);
        assert_eq!(next_unresolved_from(&[true, true, true, true], 3), None);
        assert_eq!(next_unresolved_from(&[], 0), None);
    }

    #[test]
    fn defenders_are_every_other_seat_in_apnap_order() {
        let players = seats(&[5, 6, 7, 8]);
        assert_eq!(defending_player_ids_of(&players, 0), vec![6, 7, 8]);
        // Rotating the active player rotates the order, it does not just reverse it.
        assert_eq!(defending_player_ids_of(&players, 2), vec![8, 5, 6]);
        assert_eq!(defending_player_ids_of(&seats(&[5, 6]), 1), vec![5]);
    }

    #[test]
    fn defenders_exclude_players_who_have_lost() {
        let mut players = seats(&[5, 6, 7]);
        players[1].has_lost = true;
        assert_eq!(defending_player_ids_of(&players, 0), vec![7]);
        // With every opponent gone there is no defending player at all.
        players[2].has_lost = true;
        assert!(defending_player_ids_of(&players, 0).is_empty());
    }
}
