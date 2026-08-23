use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use tricerules_cards::primitives::{
    ActivatedAbilityDef, Color, ContinuousEffectKind, CounterKind, CreatureScopeFilter,
    DamagePreventionAdditionalEffect, EffectDuration, GameCondition, Keyword, LibraryBottomOrder,
    ManaAmount, ManaSpendingRestriction, SearchDestination, TargetFilter, TriggerCondition,
    TriggeredAbilityDef,
};
use tricerules_cards::primitives::{PlayerRecipient, ResolutionBranchDef};
use tricerules_cards::{CardFace, ManaCost};
use tricerules_proto::ruled::v1::{ChoiceKind, RuledEvent, TokenCreated};

pub type PlayerId = i32;
pub type ObjectId = u32;

/// The exact activated ability on the exact incarnation of a permanent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActivationUseKey {
    pub object_id: ObjectId,
    pub zone_change_generation: u64,
    pub face_change_generation: u64,
    pub ability_index: usize,
}

/// A printed or copied triggered ability that has consumed a persistent "triggers only once"
/// allowance on one battlefield object incarnation. Card/face/ability identity prevents an
/// unrelated ability at the same index from inheriting the usage after a copy or face change;
/// control and turn changes deliberately do not participate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TriggeredOnceKey {
    pub object_id: ObjectId,
    pub zone_change_generation: u64,
    pub card_id: String,
    pub face_index: usize,
    pub ability_index: usize,
}

/// Generation-aware identity for the distinct permanent observed by a trigger event. The
/// controller snapshot supplies CR 608.2h last known information if that permanent is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerObjectRef {
    pub object_id: ObjectId,
    pub zone_change_generation: u64,
    pub controller_at_event: PlayerId,
}

/// Generation-aware identity for the spell or ability whose target-selection event created a
/// trigger. Physical spells may reuse their object id after leaving and being cast again, so they
/// carry the stack-entry zone-change generation; virtual ability/copy ids are globally unique and
/// therefore use `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackObjectRef {
    pub object_id: ObjectId,
    pub zone_change_generation: Option<u64>,
}

/// Event-time facts carried by a triggered ability from collection through resolution. Keeping
/// these together prevents target publication, target validation, and effect resolution from
/// reconstructing relationships after objects detach, change controller, or leave a zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TriggerContext {
    pub affected_player: Option<PlayerId>,
    pub observed_object: Option<TriggerObjectRef>,
    pub targeting_stack_object: Option<StackObjectRef>,
    pub attacking_player: Option<PlayerId>,
    pub defending_player: Option<PlayerId>,
}

/// The game entity an Aura or Equipment is attached to. Players are represented explicitly;
/// their numeric ids must never be confused with engine object ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentRecipient {
    Object(ObjectId),
    Player(PlayerId),
}

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
    /// CR 613 layer-2 base controller before continuous effects. Reanimation and other
    /// battlefield-entry instructions set this value; it resets to `owner` whenever the object
    /// leaves the battlefield (CR 400.7 — new object).
    pub base_controller: PlayerId,
    /// CR 110.2 current controller, materialized from layer 2 so turn, combat, trigger,
    /// legal-action, and per-player zone indexes all consume one authoritative value.
    /// `reindex_battlefield_control` updates this cache after continuous effects change and marks
    /// a creature summoning sick when the value changes (CR 302.6). `PlayerState::battlefield` is
    /// the corresponding control index; other per-player zone lists are ownership indexes.
    pub controller: PlayerId,
    pub card_id: String,
    /// CR 707.2 layer-1 values replacing the physical card's printed face while this object
    /// remains on the battlefield. Physical identity stays in `card_id`.
    pub copiable_values: Option<CopiableValues>,
    /// Incremented whenever a copy snapshot is installed. Intrinsic replacement identity uses
    /// this revision so abilities acquired from the copied face are evaluated once.
    pub copy_revision: u64,
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
    /// CR 613.7c timestamp shared by all present counters of each kind. New counters of an
    /// existing kind receive the timestamp of the first; removing counters preserves it until the
    /// last one leaves.
    pub counter_timestamps: BTreeMap<CounterKind, u64>,
    /// CR 303.4 / CR 301.5 / 702.6: the object or player this Aura or Equipment is attached to.
    /// Equipment uses only [`AttachmentRecipient::Object`]. `None` means no attachment. Cleared
    /// on every battlefield exit under the CR 400.7 zone-change funnel.
    pub attached_to: Option<AttachmentRecipient>,
    /// CR 701.19: number of regeneration shields on this permanent. Each shield is a replacement
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
    /// CR 708: use the universal face-down permanent values in layer 1b while this object is on
    /// the battlefield. Its physical identity remains in `card_id` and is private to its current
    /// controller until the object is turned face up or revealed as it leaves the battlefield.
    pub face_down: bool,
    /// Present only while this exact object remains exiled after its Adventure face resolved.
    pub adventure_cast_permission: Option<AdventureCastPermission>,
}

/// Owned CR 707.2 copiable values. Registry identity remains available for resolving copied
/// abilities, while the cloned face also represents registry-backed token definitions directly.
#[derive(Debug, Clone)]
pub struct CopiableValues {
    pub source_card_id: String,
    pub source_face_index: usize,
    pub face: CardFace,
    /// CR 707.2 / 709.5: copied Rooms retain both printed door definitions. Unlock
    /// designations are status, not copiable values, so they live in [`GameState::room_states`].
    pub room_faces: Option<Vec<CardFace>>,
    pub display_name: String,
}

/// CR 709.5 battlefield designation state for one Room permanent. Door indexes are its stable
/// copiable placement; the designations themselves are reset by every zone change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RoomState {
    pub unlocked: [bool; 2],
}

impl RoomState {
    pub fn unlocked_indices(self) -> impl Iterator<Item = usize> {
        self.unlocked
            .into_iter()
            .enumerate()
            .filter_map(|(index, unlocked)| unlocked.then_some(index))
    }

    pub fn fully_unlocked(self) -> bool {
        self.unlocked.into_iter().all(|unlocked| unlocked)
    }
}

/// Runtime scope of one damage-prevention effect. Player ids use the engine's existing widened
/// `ObjectId` convention, so `Recipient` covers both players and permanents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamagePreventionScope {
    Recipient(ObjectId),
    /// One exact CR 400.7 permanent generation, and only for combat damage. ObjectIds are reused
    /// across zone changes for relay identity, so the generation is part of the rules identity.
    CombatRecipient {
        object_id: ObjectId,
        zone_change_generation: u64,
    },
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

/// A turn-scoped CR 614 replacement bound to one exact permanent generation. ObjectIds remain
/// stable across zone changes for relay identity, so the generation is part of the rules object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveDeathReplacement {
    pub object_id: ObjectId,
    pub zone_change_generation: u64,
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

    pub fn counter_timestamp(&self, kind: CounterKind) -> Option<u64> {
        (self.counter_count(kind) > 0)
            .then(|| self.counter_timestamps.get(&kind).copied())
            .flatten()
    }

    pub fn has_any_counter(&self) -> bool {
        self.counters.values().any(|count| *count > 0)
    }

    /// Add counters and retain the timestamp of the first counter of this kind as required by
    /// CR 613.7c. Saturation matches the engine's existing bounded counter arithmetic.
    pub fn add_counters(&mut self, kind: CounterKind, amount: u32, timestamp: u64) {
        if amount == 0 {
            return;
        }
        let count = self.counters.entry(kind).or_insert(0);
        *count = count.saturating_add(amount);
        self.counter_timestamps.entry(kind).or_insert(timestamp);
    }

    /// Set the number of `kind` counters, dropping the map entry when `n` is 0 so an emptied
    /// counter kind never lingers (keeps the map minimal and iteration deterministic).
    pub fn set_counter(&mut self, kind: CounterKind, n: u32) {
        if n == 0 {
            self.counters.remove(&kind);
            self.counter_timestamps.remove(&kind);
        } else {
            self.counters.insert(kind, n);
            self.counter_timestamps.entry(kind).or_insert(0);
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
    /// CR 106.6 mana that cannot be merged into the unrestricted aggregate. Entries retain the
    /// activation that produced them for exact undo; events aggregate equal `restriction_group_id`
    /// values for the adjacent UI columns.
    pub restricted_mana: Vec<RestrictedManaContribution>,
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
            restricted_mana: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedManaContribution {
    pub restriction_group_id: u32,
    pub amount: ManaAmount,
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
    pub ability: TriggeredAbilityDef,
    pub ability_text: String,
    /// The event's affected players and observed objects, kept separate from CR 115 targets and
    /// preserved while this trigger waits for APNAP ordering or target selection.
    pub trigger_context: TriggerContext,
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
    pub ability: TriggeredAbilityDef,
    pub ability_text: String,
    pub card_id: String,
    pub controller: PlayerId,
    /// CR 603.5: an optional triggered ability may be declined before it is put on the stack.
    pub may: bool,
    /// Mirror of [`StackItem::trigger_context`], carried across target selection so a targeted
    /// event-dependent trigger keeps its event references when it finally reaches the stack.
    pub trigger_context: TriggerContext,
}

/// A resolution or state-based action parked while one player answers an engine-authored choice.
/// While present it blocks priority, and the deciding player's logged `SubmitResolutionChoice`
/// command drives it forward. Domain-specific continuation state lives in
/// [`ResolutionContinuation`], so unrelated choice families cannot accidentally coexist.
#[derive(Debug, Clone)]
pub struct PendingManaPayment {
    /// Stack object the resolving soft counter will counter if the player declines.
    pub target_spell_id: ObjectId,
    /// Generic mana required to preserve that spell.
    pub generic_mana_cost: u32,
    /// Printed colored/colorless cost for a resolution branch. Empty for legacy generic soft
    /// counters, whose requirement remains in `generic_mana_cost`.
    pub mana_cost: ManaCost,
    /// First entry in `undoable_mana_abilities` created while this payment prompt was active.
    /// Undo and Decline may rewind entries at or after this boundary, never earlier float.
    pub undo_history_start: usize,
}

#[derive(Debug, Clone)]
pub struct PendingWardPayment {
    pub target: StackObjectRef,
    pub stage: PendingWardPaymentStage,
}

#[derive(Debug, Clone)]
pub enum PendingWardPaymentStage {
    Mana(PendingManaPayment),
    Discard {
        candidate_generations: Vec<(ObjectId, u64)>,
    },
}

#[derive(Debug, Clone)]
pub struct PendingResolutionBranch {
    pub optional: bool,
    pub chooser: PlayerRecipient,
    pub branches: Vec<ResolutionBranchDef>,
    pub stage: PendingResolutionBranchStage,
}

#[derive(Debug, Clone)]
pub enum PendingResolutionBranchStage {
    Selecting,
    PayingMana {
        selected_branch: usize,
        payment: PendingManaPayment,
    },
    PayingObjects {
        selected_branch: usize,
    },
}

/// Stack-resolution context shared only by continuation families that actually resume a stack
/// item. Non-stack choices such as the legend rule therefore need no synthetic `StackItem`.
#[derive(Debug, Clone)]
pub struct ParkedStackResolution {
    pub item: StackItem,
    /// Index of the next primitive effect to execute after this choice. `None` is retained for
    /// continuations that own their complete resolution flow.
    pub resume_effect_index: Option<u32>,
}

impl ParkedStackResolution {
    pub fn new(item: StackItem) -> Self {
        Self {
            item,
            resume_effect_index: None,
        }
    }
}

/// Common publication and validation data carried by every resolution choice.
#[derive(Debug, Clone)]
pub struct PendingResolutionPresentation {
    pub source_object_id: ObjectId,
    pub candidates: Vec<ObjectId>,
    pub min: u32,
    pub max: u32,
    pub ordered: bool,
    pub prompt: String,
    pub choice_kind: ChoiceKind,
    pub unique_names: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingLibraryPartitionStage {
    ChooseDestination,
    OrderTop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingLibraryPartitionKind {
    Scry,
    Surveil,
    Look,
}

#[derive(Debug, Clone)]
pub enum PendingLibraryLookStage {
    ChooseToHand {
        looked_at: Vec<ObjectId>,
        bottom_order: LibraryBottomOrder,
    },
    OrderBottom,
}

/// The exact work to resume after a resolution choice. Each variant owns only the metadata its
/// handler consumes; engine-owned string sentinels and unrelated optional fields are forbidden.
#[derive(Debug, Clone)]
pub enum ResolutionContinuation {
    Custom {
        stack: ParkedStackResolution,
        key: String,
        step: u32,
        scratch: Vec<ObjectId>,
    },
    ManaPayment {
        stack: ParkedStackResolution,
        payment: PendingManaPayment,
    },
    AuthoredBranch {
        stack: ParkedStackResolution,
        branch: PendingResolutionBranch,
    },
    WardPayment {
        stack: ParkedStackResolution,
        ward: PendingWardPayment,
    },
    HandChoice {
        stack: ParkedStackResolution,
        hand_choice: PendingHandChoice,
    },
    Sacrifice {
        stack: ParkedStackResolution,
    },
    CopyTargets {
        stack: ParkedStackResolution,
        copy_source_object_id: ObjectId,
    },
    SearchLibrary {
        stack: ParkedStackResolution,
        destination: SearchDestination,
        shuffle: bool,
        reveal: bool,
    },
    LibraryPartition {
        stack: ParkedStackResolution,
        looked_at: Vec<ObjectId>,
        stage: PendingLibraryPartitionStage,
        kind: PendingLibraryPartitionKind,
    },
    LibraryLook {
        stack: ParkedStackResolution,
        stage: PendingLibraryLookStage,
    },
    ManifestDread {
        stack: ParkedStackResolution,
        looked_at: Vec<ObjectId>,
    },
    EntryCopySource {
        stack: ParkedStackResolution,
    },
    EntryReplacement {
        stack: ParkedStackResolution,
    },
    DamageReplacement {
        stack: ParkedStackResolution,
        effect_ids: Vec<u32>,
    },
    LegendKeep,
}

impl ResolutionContinuation {
    pub fn stack(&self) -> Option<&ParkedStackResolution> {
        match self {
            Self::Custom { stack, .. }
            | Self::ManaPayment { stack, .. }
            | Self::AuthoredBranch { stack, .. }
            | Self::WardPayment { stack, .. }
            | Self::HandChoice { stack, .. }
            | Self::Sacrifice { stack }
            | Self::CopyTargets { stack, .. }
            | Self::SearchLibrary { stack, .. }
            | Self::LibraryPartition { stack, .. }
            | Self::LibraryLook { stack, .. }
            | Self::ManifestDread { stack, .. }
            | Self::EntryCopySource { stack }
            | Self::EntryReplacement { stack }
            | Self::DamageReplacement { stack, .. } => Some(stack),
            Self::LegendKeep => None,
        }
    }

    pub fn stack_mut(&mut self) -> Option<&mut ParkedStackResolution> {
        match self {
            Self::Custom { stack, .. }
            | Self::ManaPayment { stack, .. }
            | Self::AuthoredBranch { stack, .. }
            | Self::WardPayment { stack, .. }
            | Self::HandChoice { stack, .. }
            | Self::Sacrifice { stack }
            | Self::CopyTargets { stack, .. }
            | Self::SearchLibrary { stack, .. }
            | Self::LibraryPartition { stack, .. }
            | Self::LibraryLook { stack, .. }
            | Self::ManifestDread { stack, .. }
            | Self::EntryCopySource { stack }
            | Self::EntryReplacement { stack }
            | Self::DamageReplacement { stack, .. } => Some(stack),
            Self::LegendKeep => None,
        }
    }

    pub fn mana_payment(&self) -> Option<&PendingManaPayment> {
        match self {
            Self::ManaPayment { payment, .. } => Some(payment),
            Self::AuthoredBranch {
                branch:
                    PendingResolutionBranch {
                        stage: PendingResolutionBranchStage::PayingMana { payment, .. },
                        ..
                    },
                ..
            } => Some(payment),
            Self::WardPayment {
                ward:
                    PendingWardPayment {
                        stage: PendingWardPaymentStage::Mana(payment),
                        ..
                    },
                ..
            } => Some(payment),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PendingResolution {
    pub deciding_player: PlayerId,
    pub presentation: PendingResolutionPresentation,
    pub continuation: ResolutionContinuation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandCardAction {
    Discard,
    Exile,
}

#[derive(Debug, Clone)]
pub struct PendingHandChoice {
    pub affected_player: PlayerId,
    pub action: HandCardAction,
    pub candidate_generations: Vec<(ObjectId, u64)>,
    pub draw_after: u32,
    pub draw_only_if_discarded: bool,
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
        copy_revision: u64,
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
    /// The door selected while casting a Room permanent spell. Other battlefield-entry paths
    /// carry `None`; entering as a copy suppresses the designation at commitment.
    pub unlock_room_door: Option<usize>,
    /// X chosen for the entering permanent spell. Non-spell entry paths carry zero.
    pub chosen_x: u32,
    /// Public life totals captured when this event is proposed. Simultaneous entries receive the
    /// same snapshot, so replacement ordering cannot retroactively change an entry predicate.
    pub player_life_snapshot: BTreeMap<PlayerId, i32>,
    pub tapped: bool,
    /// Counter state accumulated by entry replacement effects before zone commitment.
    pub entry_counters: BTreeMap<CounterKind, u32>,
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
        attached_to: Option<AttachmentRecipient>,
    },
    ResolutionEffect {
        owner: PlayerId,
        spell_label: String,
        object_label: String,
    },
    LibrarySearch {
        owner: PlayerId,
        card_label: String,
        shuffle: bool,
        resume_effect_index: Option<u32>,
    },
    ManifestDread {
        owner: PlayerId,
        other_object_id: Option<ObjectId>,
        chosen_library_position: u32,
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
    /// Present while an `EntersAsCopy` application is waiting for its source selection.
    pub copy_source_effect: Option<EntryReplacementEffectId>,
    pub completion: BattlefieldEntryCompletion,
}

#[derive(Debug, Clone)]
pub struct ChosenMode {
    pub mode_index: usize,
    pub targets: Vec<StackTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackTarget {
    pub object_id: ObjectId,
    pub group_index: u32,
    pub damage_amount: u32,
    /// Authoritative presentation domain for clients. ObjectId and PlayerId share integers.
    pub kind: i32,
    /// Generation captured when an object target was chosen. Player targets have no generation.
    pub zone_change_generation: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct StackItem {
    pub id: ObjectId,
    pub controller: PlayerId,
    pub card_id: String,
    pub targets: Vec<StackTarget>,
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
    /// Index in the source's effective activated- or triggered-ability list. `None` for spells.
    pub ability_index: Option<usize>,
    /// Snapshot of an activated ability's complete definition. Granted abilities resolve from
    /// this value after their granting effect has disappeared.
    pub activated_ability: Option<ActivatedAbilityDef>,
    /// Snapshot of a triggered ability's complete definition. Printed and granted triggers both
    /// resolve from this value after their source or granting effect has disappeared.
    pub triggered_ability: Option<TriggeredAbilityDef>,
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
    /// Atomic modal choices in printed order. Empty for nonmodal spells and abilities.
    pub chosen_modes: Vec<ChosenMode>,
    /// Resolution branches already answered, keyed by their index in the original effect list.
    /// `None` records an optional decline; `Some(i)` records the chosen authored branch.
    pub resolution_branch_choices: BTreeMap<u32, Option<usize>>,
    /// Event-time player and object identity for triggered abilities. This includes an affected
    /// player (Howling Mine), an observed object distinct from CR 115 targets, and attack
    /// participants. Empty for spells and activated abilities. Trigger context never changes who
    /// controls the stack item.
    pub trigger_context: TriggerContext,
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
    /// Creatures matching a creature-scope filter (CR 613) — resolved from a
    /// [`CreatureScopeFilter`] when
    /// the source's static ability fires. Evaluated dynamically each characteristics query so
    /// creatures entering or gaining counters after the effect starts can become affected.
    /// `filter.controller` relates each creature to `reference_player`; source-bound static effects
    /// replace that reference with the source's current controller. The remaining predicates use
    /// derived characteristics, effective copiable names, current counters, and authoritative
    /// combat membership. `exclude` is the physical source for "other ... creatures" abilities.
    CreaturesMatching {
        reference_player: PlayerId,
        filter: CreatureScopeFilter,
        exclude: Option<ObjectId>,
    },
    /// Permanents matched by a resolving rule-changing effect. Unlike characteristic-changing
    /// one-shot scopes, this set remains dynamic for the effect's duration (CR 611.2c).
    PermanentsMatching {
        reference_player: PlayerId,
        filter: TargetFilter,
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
    /// A dynamically reevaluated public condition. Static self-modifiers use the source's current
    /// layer-2 controller as the reference player; existing unconditional effects leave this empty.
    pub condition: Option<GameCondition>,
    pub duration: EffectDuration,
    /// `command_index` at creation; used for layer sublayer timestamp ordering (CR 613.7).
    pub timestamp: u64,
}

/// A one-shot delayed triggered ability created by a resolving effect (CR 603.7).
#[derive(Debug, Clone)]
pub struct ActiveDelayedTrigger {
    pub controller: PlayerId,
    pub card_id: String,
    pub card_name: String,
    pub source_face_index: usize,
    pub watched: TriggerObjectRef,
    pub ability: TriggeredAbilityDef,
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
    /// `None` means the contribution was added to the ordinary aggregate pool. Restricted
    /// activations retain their group so undo cannot remove an unrestricted pip of the same color.
    pub restriction_group_id: Option<u32>,
}

/// Public, identity-free facts accumulated during one turn. Counts are retained rather than
/// booleans because some cards ask whether anything happened while others use the exact total.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerTurnRecord {
    pub spells_cast: u32,
    pub cards_drawn: u32,
    pub attacked: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TurnRecord {
    pub spells_cast: u32,
    pub creatures_died: u32,
    pub by_player: BTreeMap<PlayerId, PlayerTurnRecord>,
}

impl TurnRecord {
    pub fn player(&self, player: PlayerId) -> PlayerTurnRecord {
        self.by_player.get(&player).copied().unwrap_or_default()
    }

    pub fn player_mut(&mut self, player: PlayerId) -> &mut PlayerTurnRecord {
        self.by_player.entry(player).or_default()
    }
}

/// Engine-owned event memory. Cleanup rolls the completed turn into `previous` and opens a fresh
/// `current` record, so turn-bound conditions share one lifecycle instead of adding ad hoc fields.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TurnHistory {
    pub current: TurnRecord,
    pub previous: TurnRecord,
}

impl TurnHistory {
    pub fn finish_turn(&mut self) {
        self.previous = std::mem::take(&mut self.current);
    }
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
    /// Last-known derived colors and types for source-quality checks such as protection. These are
    /// generation-scoped so a leave-and-return object cannot rewrite an older ability's source.
    pub last_known_colors_by_generation: HashMap<(ObjectId, u64), Vec<Color>>,
    pub last_known_types_by_generation: HashMap<(ObjectId, u64), Vec<String>>,
    /// Last-known derived controller keyed by source object and generation. Resolving abilities
    /// use this for source-controller wording after the source leaves the battlefield.
    pub last_known_controller_by_generation: HashMap<(ObjectId, u64), PlayerId>,
    /// The object an Aura or Equipment source was attached to as that source last left the
    /// battlefield, keyed by the source generation. The value carries the attached object's
    /// generation so an old ability cannot affect a card that left and returned under the same
    /// relay-compatible ObjectId (CR 400.7, 608.2h).
    pub last_known_attached_object_by_generation: HashMap<(ObjectId, u64), (ObjectId, u64)>,
    /// Monotonic per-object generation incremented on every zone change. ObjectIds remain stable
    /// for relay compatibility, while this generation preserves CR 400.7 identity semantics for
    /// effects that resolve after a source leaves and returns.
    pub zone_change_generation: HashMap<ObjectId, u64>,
    /// Incremented whenever a battlefield permanent changes face/status in place.
    pub face_change_generation: HashMap<ObjectId, u64>,
    /// Public CR 709.5 designations for battlefield Rooms. Absence means the object is not a
    /// Room permanent; the zone-change funnel removes entries on departure under CR 400.7.
    pub room_states: HashMap<ObjectId, RoomState>,
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
    /// Successful limited activations in the current turn, keyed to full object and ability
    /// identity. Control changes do not affect the key (CR 602.5b).
    pub activation_uses_this_turn: HashMap<ActivationUseKey, u32>,
    /// Persistent "triggers only once" usage. Generation-aware keys make leave-and-return a
    /// fresh object without copying or resetting usage on control changes or turn boundaries.
    pub triggered_once: HashSet<TriggeredOnceKey>,
    pub turn_history: TurnHistory,
    /// Active combat, if in declare/damage
    pub combat: Option<CombatState>,
    /// If set, game is over; winning player
    pub winner: Option<PlayerId>,
    /// CR 514.1: active player who must discard during their cleanup step, if any.
    pub cleanup_discard_player: Option<PlayerId>,
    /// CR 514.3: a trigger occurred during cleanup, so players receive priority and another
    /// cleanup step follows once the stack is empty and everyone passes.
    pub cleanup_priority_active: bool,
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
    /// One-shot delayed triggers waiting for their event boundary.
    pub active_delayed_triggers: Vec<ActiveDelayedTrigger>,
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
    /// CR 502.3 / 611.2a rules-changing effects that suppress one permanent's next controller
    /// untap. The generation is part of identity because relay-compatible ObjectIds survive zone
    /// changes (CR 400.7). A set intentionally coalesces repeated applications: every identical
    /// "next untap step" restriction expires during the same applicable step.
    pub skip_next_untap: HashSet<(ObjectId, u64)>,
    /// Active CR 615 prevention effects. Healing Salve and Fog are both represented here so every
    /// producer enters one event pipeline and finite effects have stable opaque identities.
    pub damage_prevention_effects: Vec<ActiveDamagePrevention>,
    /// Active CR 615.12 prohibitions, deliberately separate from prevention effects.
    pub damage_prevention_prohibitions: Vec<DamagePreventionProhibition>,
    pub next_damage_prevention_effect_id: u32,
    /// Active "if this permanent would die this turn, exile it instead" replacements.
    pub death_replacement_effects: Vec<ActiveDeathReplacement>,
    /// Opaque ids exposed by replacement-order prompts. Separate from effect identities so stale
    /// submissions cannot name a newly-applicable effect after recomputation.
    pub next_replacement_application_id: u32,
    /// Session-lifetime interning table for structurally identical mana restrictions. The
    /// one-based index is the stable public group id used by commands and UI snapshots.
    pub mana_restrictions: Vec<ManaSpendingRestriction>,
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

    /// Whether two players are opponents (CR 102.2-102.3).
    ///
    /// The current engine has no teams, so every distinct player is an opponent. Keep this
    /// relationship separate from player validity, seat order, and [`PlayerState::has_lost`]:
    /// callers that enumerate legal or affected players remain responsible for those concerns.
    /// When a concrete team format is implemented, team membership belongs behind this method.
    pub(crate) fn are_opponents(&self, first: PlayerId, second: PlayerId) -> bool {
        first != second
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

    pub(crate) fn stage_delayed_control_loss(
        &mut self,
        transitions: &[(ObjectId, PlayerId, Option<PlayerId>)],
    ) {
        let mut waiting = std::mem::take(&mut self.active_delayed_triggers);
        let mut fired = Vec::new();
        for delayed in waiting.drain(..) {
            let matches = delayed.ability.trigger == TriggerCondition::WhenControllerLosesControlOf
                && transitions
                    .iter()
                    .any(|&(object_id, old_controller, new_controller)| {
                        object_id == delayed.watched.object_id
                            && self
                                .zone_change_generation
                                .get(&object_id)
                                .copied()
                                .unwrap_or(0)
                                == delayed.watched.zone_change_generation
                            && old_controller == delayed.controller
                            && new_controller != Some(delayed.controller)
                    });
            if matches {
                fired.push(delayed);
            } else {
                self.active_delayed_triggers.push(delayed);
            }
        }
        self.stage_delayed_batch(fired);
    }

    pub(crate) fn take_next_end_step_delayed(&mut self) -> Vec<ActiveDelayedTrigger> {
        let mut waiting = std::mem::take(&mut self.active_delayed_triggers);
        let mut fired = Vec::new();
        for delayed in waiting.drain(..) {
            if delayed.ability.trigger == TriggerCondition::AtBeginningOfNextEndStep {
                fired.push(delayed);
            } else {
                self.active_delayed_triggers.push(delayed);
            }
        }
        fired
    }

    fn stage_delayed_batch(&mut self, delayed: Vec<ActiveDelayedTrigger>) {
        let mut triggers = delayed
            .into_iter()
            .map(|delayed| {
                let object_id = self.next_object_id;
                self.next_object_id += 1;
                StagedTrigger {
                    object_id,
                    source_permanent_id: delayed.watched.object_id,
                    source_face_index: delayed.source_face_index,
                    source_zone_change: delayed.watched.zone_change_generation,
                    source_face_change: 0,
                    card_id: delayed.card_id,
                    card_name: delayed.card_name,
                    controller: delayed.controller,
                    ability_index: 0,
                    ability_text: delayed.ability.text.clone(),
                    trigger_context: TriggerContext {
                        observed_object: Some(delayed.watched),
                        ..TriggerContext::default()
                    },
                    may: delayed.ability.may,
                    ability: delayed.ability,
                }
            })
            .collect::<Vec<_>>();
        triggers.sort_by_key(|trigger| self.apnap_rank(trigger.controller));
        if !triggers.is_empty() {
            self.staged_trigger_groups
                .push_back(StagedTriggerGroup { triggers });
        }
    }

    /// Every defending player under the current duel/free-for-all combat policy (CR 506.2): each
    /// nonactive player still in the game, in APNAP order (CR 101.4). Exactly one element in 1v1.
    /// Team variants and multiplayer options choose defenders differently and must replace this
    /// policy rather than treating seat-generic ordering as complete format support.
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
