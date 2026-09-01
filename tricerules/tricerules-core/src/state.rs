use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use tricerules_cards::primitives::{
    ActivatedAbilityDef, CardResultAction, CardSearchZone, CardTypeFilter,
    CastCostReceiptCondition, Color, ConditionalSearchDestination, ContinuousEffectKind,
    CounterKind, CreatureScopeFilter, DamagePreventionAdditionalEffect,
    DelayedTokenSacrificeTiming, EffectDuration, GameCondition, Keyword, LibraryBottomOrder,
    LibraryPlacement, ManaAmount, ManaSpendingRestriction, PermanentTypeFilter, SearchDestination,
    TargetFilter, TriggeredAbilityDef, ZoneCardFilter,
};
use tricerules_cards::primitives::{PlayerRecipient, ResolutionBranchDef};
use tricerules_cards::{is_creature_type, CardFace, ChoiceId, ManaCost, ManaSymbol, ModeId};
use tricerules_proto::ruled::v1::{ChoiceKind, RuledEvent, TokenCreated};

pub type PlayerId = i32;
pub type ObjectId = u32;

/// Committed CR 701.68 operation. A forced instruction can complete without a recipient;
/// optional payments always have one. Never infer payment from the surviving counter bag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlightReceipt {
    pub player: PlayerId,
    pub count: u32,
    pub creature: Option<TriggerObjectRef>,
}

/// Private rules-only snapshot of one card that was actually moved by a cost or instruction.
/// `matched_card_types` is captured at the action boundary, so later predicates never inspect a
/// destination zone or a newer object generation.
#[derive(Debug, Clone)]
pub(crate) struct CardResultEntry {
    pub action: CardResultAction,
    pub affected_player: PlayerId,
    pub object_id: ObjectId,
    pub zone_change_generation: u64,
    pub matched_card_types: Vec<CardTypeFilter>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CardResultCohort {
    pub cards: Vec<CardResultEntry>,
}

/// Private typed output of one primitive resolution instruction. Card cohorts and semantic
/// receipts intentionally share this immediate-result boundary: both are consumed only by the
/// following instruction and never serialized to clients.
#[derive(Debug, Clone, Default)]
pub(crate) struct EffectResult {
    pub cards: Vec<CardResultEntry>,
    pub selected_objects: Vec<TriggerObjectRef>,
    pub receipt: Option<ResolutionReceipt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolutionReceipt {
    CounterUnlessPaid { paid: bool },
}

impl From<CardResultCohort> for EffectResult {
    fn from(cohort: CardResultCohort) -> Self {
        Self {
            cards: cohort.cards,
            selected_objects: Vec::new(),
            receipt: None,
        }
    }
}

/// The exact activated ability on the exact incarnation of a permanent.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActivationUseKey {
    pub object_id: ObjectId,
    pub zone_change_generation: u64,
    pub definition: AbilityDefinitionId,
}

/// One activated ability on one CR 400.7 permanent object. Unlike [`ActivationUseKey`], this key
/// deliberately omits turn and face-status identity: only a zone change creates a fresh object
/// and restores a once-per-object allowance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PersistentActivationUseKey {
    pub object_id: ObjectId,
    pub zone_change_generation: u64,
    pub definition: AbilityDefinitionId,
}

/// Authored ability slot, independent of display names and flattened live ability indexes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AbilityDefinitionId {
    pub card_id: String,
    pub face_id: tricerules_cards::CardFaceId,
    pub ability_path: Vec<tricerules_cards::AbilityId>,
}

/// CR 113.2c identity of an ability occurrence. Infernal Scarring's static grant and
/// Abnormal Endurance's resolving grant must remain distinct even when their text is identical.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TriggerAbilityOrigin {
    Printed(AbilityDefinitionId),
    StaticGrant {
        source_id: ObjectId,
        source_zone_change: u64,
        definition: AbilityDefinitionId,
    },
    ResolvingGrant(u64),
}

/// One ability on one CR 400.7 source incarnation. Control, turn and face-status generations
/// deliberately do not participate; the authored face belongs to the ability's provenance.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TriggerUseKey {
    pub object_id: ObjectId,
    pub zone_change_generation: u64,
    pub ability_origin: TriggerAbilityOrigin,
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
    /// CR 400.7e public-zone incarnation reached by a self zone-change trigger.
    /// Hoarding Recluse and Myr Retriever exclude this card, not every later incarnation.
    pub source_after_zone_change: Option<TriggerObjectRef>,
    pub affected_player: Option<PlayerId>,
    pub observed_object: Option<TriggerObjectRef>,
    pub targeting_stack_object: Option<StackObjectRef>,
    pub attacking_player: Option<PlayerId>,
    pub defending_player: Option<PlayerId>,
    /// The player or planeswalker this source attacked. Battles deliberately populate neither:
    /// effects such as Scorch Spitter name only a player or planeswalker.
    pub attacked_player: Option<PlayerId>,
    pub attacked_planeswalker: Option<TriggerObjectRef>,
}

/// The game entity an Aura or Equipment is attached to. Players are represented explicitly;
/// their numeric ids must never be confused with engine object ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentRecipient {
    Object(ObjectId),
    Player(PlayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExilePlayPermissionScope {
    /// Cast one named face, used by Adventure (CR 715.3d).
    CastFace(usize),
    /// Play any otherwise-available spell or land face of the card.
    PlayCard,
    /// Cast any otherwise available spell face, but never play a land (Warp).
    CastCard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExilePlayPermissionOrigin {
    Effect,
    Warp,
}

/// The base mana cost authorized by an exile permission. Ordinary permissions retain the
/// printed face cost; effect-created alternatives replace only that base cost before the shared
/// CR 601 additional-cost and cost-modification pipeline runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExilePermissionCastCost {
    PrintedManaCost,
    AlternativeManaCost(ManaCost),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExilePlayPermissionGrant {
    pub scope: ExilePlayPermissionScope,
    pub cast_cost: ExilePermissionCastCost,
    pub origin: ExilePlayPermissionOrigin,
    pub available_after_turn_instance: Option<u64>,
    pub until_end_of_next_turn: bool,
}

impl ExilePlayPermissionGrant {
    pub fn printed(scope: ExilePlayPermissionScope, until_end_of_next_turn: bool) -> Self {
        Self {
            scope,
            cast_cost: ExilePermissionCastCost::PrintedManaCost,
            origin: ExilePlayPermissionOrigin::Effect,
            available_after_turn_instance: None,
            until_end_of_next_turn,
        }
    }
}

/// One deterministic, generation-aware permission entry. Entries created by one resolving
/// spell or ability share a group id and source label for the client presentation snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveExilePlayPermission {
    pub group_id: u64,
    pub player_id: PlayerId,
    pub source_label: String,
    pub object_id: ObjectId,
    pub zone_change_generation: u64,
    pub scope: ExilePlayPermissionScope,
    pub cast_cost: ExilePermissionCastCost,
    pub origin: ExilePlayPermissionOrigin,
    /// Warp is unavailable during the turn in which its delayed trigger exiled the card.
    pub available_after_turn_instance: Option<u64>,
    /// Monotonic turn instance whose cleanup ends the permission. `None` means while exiled.
    pub expires_at_cleanup_turn_instance: Option<u64>,
}

impl ActiveExilePlayPermission {
    pub fn available_on_turn(&self, turn_instance: u64) -> bool {
        self.available_after_turn_instance
            .is_none_or(|turn| turn_instance > turn)
    }
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
    /// CR 111.3: intrinsic token identity, independent of registry membership and of later
    /// copy effects. Ordinary token makers and copy-token makers both populate this snapshot.
    /// It remains present after a zone change until the token ceases to exist.
    pub token_origin: Option<CopiableValues>,
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
    /// CR 613.7c timestamp shared by all present counters of each kind. Adding counters
    /// refreshes this timestamp; removing counters preserves it until the
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
}

/// Owned CR 707.2 copiable values. Registry identity remains available for resolving copied
/// abilities, while the cloned face also represents registry-backed token definitions directly.
#[derive(Debug, Clone)]
pub struct CopiableValues {
    /// Definition provenance when available; empty for inline or anonymous values.
    /// Rules execution uses the owned face, never requires this registry lookup.
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
    pub fn is_token(&self) -> bool {
        self.token_origin.is_some()
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

    /// Add counters and refresh the timestamp of every counter of this kind as required by
    /// CR 613.7c. Saturation matches the engine's existing bounded counter arithmetic.
    pub fn add_counters(&mut self, kind: CounterKind, amount: u32, timestamp: u64) {
        if amount == 0 {
            return;
        }
        let count = self.counters.entry(kind).or_insert(0);
        *count = count.saturating_add(amount);
        self.counter_timestamps.insert(kind, timestamp);
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
    /// Unrestricted mana included in `mana_pool` that survives ordinary combat-step boundaries.
    /// This engine-private subset is never published separately; it exists only to implement
    /// effects such as firebending without changing the authoritative aggregate pool contract.
    pub retained_combat_mana: ManaPool,
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
            retained_combat_mana: ManaPool::default(),
            restricted_mana: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedManaContribution {
    pub restriction_group_id: u32,
    pub amount: ManaAmount,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
    pub presentation: Option<tricerules_proto::ruled::v1::PresentationRef>,
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
    pub presentation: Option<tricerules_proto::ruled::v1::PresentationRef>,
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
    /// Waterbending Lesson shares staged mana/object payment with activated Waterbend costs.
    pub waterbend: bool,
    /// Stack object the resolving soft counter will counter if the player declines.
    pub target_spell_id: ObjectId,
    /// Pure generic cost, staged by the client's mana-pip picker.
    pub generic_mana_cost: u32,
    /// Printed cost when it is not purely generic. Empty for generic costs, whose requirement
    /// remains in `generic_mana_cost`.
    pub mana_cost: ManaCost,
    /// First entry in `undoable_mana_abilities` created while this payment prompt was active.
    /// Undo and Decline may rewind entries at or after this boundary, never earlier float.
    pub undo_history_start: usize,
}

impl PendingManaPayment {
    /// Share the staged generic payment contract across Ward and authored resolution branches
    /// (for example Chaos Spewer and Mentor of the Meek). Do not publish a generic cost as a
    /// structured cost with a zero remainder: the client would mistake it for a finished payment.
    pub(crate) fn from_cost(
        target_spell_id: ObjectId,
        mana_cost: ManaCost,
        undo_history_start: usize,
    ) -> Self {
        let generic = mana_cost
            .pips
            .iter()
            .try_fold(0u32, |total, pip| match pip {
                ManaSymbol::Generic(amount) => total.checked_add(*amount),
                _ => None,
            });
        let (generic_mana_cost, mana_cost) = match generic {
            Some(amount) => (amount, ManaCost::default()),
            None => (0, mana_cost),
        };
        Self {
            target_spell_id,
            waterbend: false,
            generic_mana_cost,
            mana_cost,
            undo_history_start,
        }
    }
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
        candidate_generations: Vec<(ObjectId, u64)>,
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
    /// Exact card cohort produced by the primitive that parked this resolution. It remains
    /// engine-private while a player answers the choice, then becomes the next primitive's
    /// `PreviousEffect` input.
    pub(crate) previous_result: EffectResult,
}

impl ParkedStackResolution {
    pub fn new(item: StackItem) -> Self {
        Self {
            item,
            resume_effect_index: None,
            previous_result: EffectResult::default(),
        }
    }

    pub(crate) fn with_previous_result(mut self, result: EffectResult) -> Self {
        self.previous_result = result;
        self
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
    PermanentChoice {
        stack: ParkedStackResolution,
        candidate_generations: Vec<(ObjectId, u64)>,
    },
    WardPayment {
        stack: ParkedStackResolution,
        ward: PendingWardPayment,
    },
    HandChoice {
        stack: ParkedStackResolution,
        hand_choice: PendingHandChoice,
    },
    PlayerSetDiscard {
        stack: ParkedStackResolution,
        discard: PendingPlayerSetDiscard,
    },
    GraveyardChoice {
        stack: ParkedStackResolution,
        destination: tricerules_cards::primitives::GraveyardDestination,
        candidate_generations: Vec<(ObjectId, u64)>,
        spell_label: String,
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
        zones: Vec<CardSearchZone>,
        candidate_generations: Vec<(ObjectId, u64)>,
        selection_slot_candidates: Vec<Vec<ObjectId>>,
        destination: SearchDestination,
        conditional_destination: Option<ConditionalSearchDestination>,
        shuffle: bool,
        reveal: bool,
    },
    SearchZoneScope {
        stack: ParkedStackResolution,
        count: u32,
        available_zones: Vec<CardSearchZone>,
        filter: Option<ZoneCardFilter>,
        destination: SearchDestination,
        conditional_destination: Option<ConditionalSearchDestination>,
        shuffle: bool,
        reveal: bool,
    },
    OwnerLibraryPlacement {
        stack: ParkedStackResolution,
        object_id: ObjectId,
        owner: PlayerId,
        zone_change_generation: u64,
        nonbottom_placement: LibraryPlacement,
        spell_label: String,
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
    Populate {
        stack: ParkedStackResolution,
        candidate_generations: Vec<(ObjectId, u64)>,
    },
    Blight {
        stack: ParkedStackResolution,
        count: u32,
        candidate_generations: Vec<(ObjectId, u64)>,
    },
    EntryReplacement {
        stack: ParkedStackResolution,
    },
    DamageReplacement {
        stack: ParkedStackResolution,
        effect_ids: Vec<u32>,
    },
    AuraReturn {
        stack: Option<ParkedStackResolution>,
        exiled: TriggerObjectRef,
    },
    BattleProtector {
        stack: ParkedStackResolution,
    },
    AttackingTokenDefenders {
        stack: ParkedStackResolution,
        entries: Vec<TokenBattlefieldEntry>,
        logs: Vec<String>,
        chosen_defenders: Vec<tricerules_proto::ruled::v1::CombatDefenderOption>,
        current_options: Vec<tricerules_proto::ruled::v1::CombatDefenderOption>,
        delayed_sacrifice: Option<DelayedTokenSacrificeTiming>,
    },
    SiegeCast {
        stack: ParkedStackResolution,
        exiled: TriggerObjectRef,
        face_index: usize,
    },
    LegendKeep,
}

impl ResolutionContinuation {
    pub fn stack(&self) -> Option<&ParkedStackResolution> {
        match self {
            Self::Custom { stack, .. }
            | Self::ManaPayment { stack, .. }
            | Self::AuthoredBranch { stack, .. }
            | Self::PermanentChoice { stack, .. }
            | Self::WardPayment { stack, .. }
            | Self::HandChoice { stack, .. }
            | Self::PlayerSetDiscard { stack, .. }
            | Self::GraveyardChoice { stack, .. }
            | Self::Sacrifice { stack }
            | Self::CopyTargets { stack, .. }
            | Self::SearchLibrary { stack, .. }
            | Self::SearchZoneScope { stack, .. }
            | Self::OwnerLibraryPlacement { stack, .. }
            | Self::LibraryPartition { stack, .. }
            | Self::LibraryLook { stack, .. }
            | Self::ManifestDread { stack, .. }
            | Self::EntryCopySource { stack }
            | Self::Populate { stack, .. }
            | Self::Blight { stack, .. }
            | Self::EntryReplacement { stack }
            | Self::DamageReplacement { stack, .. }
            | Self::BattleProtector { stack }
            | Self::AttackingTokenDefenders { stack, .. } => Some(stack),
            Self::SiegeCast { stack, .. } => Some(stack),
            Self::AuraReturn { stack, .. } => stack.as_ref(),
            Self::LegendKeep => None,
        }
    }

    pub fn stack_mut(&mut self) -> Option<&mut ParkedStackResolution> {
        match self {
            Self::Custom { stack, .. }
            | Self::ManaPayment { stack, .. }
            | Self::AuthoredBranch { stack, .. }
            | Self::PermanentChoice { stack, .. }
            | Self::WardPayment { stack, .. }
            | Self::HandChoice { stack, .. }
            | Self::PlayerSetDiscard { stack, .. }
            | Self::GraveyardChoice { stack, .. }
            | Self::Sacrifice { stack }
            | Self::CopyTargets { stack, .. }
            | Self::SearchLibrary { stack, .. }
            | Self::SearchZoneScope { stack, .. }
            | Self::OwnerLibraryPlacement { stack, .. }
            | Self::LibraryPartition { stack, .. }
            | Self::LibraryLook { stack, .. }
            | Self::ManifestDread { stack, .. }
            | Self::EntryCopySource { stack }
            | Self::Populate { stack, .. }
            | Self::Blight { stack, .. }
            | Self::EntryReplacement { stack }
            | Self::DamageReplacement { stack, .. }
            | Self::BattleProtector { stack }
            | Self::AttackingTokenDefenders { stack, .. } => Some(stack),
            Self::SiegeCast { stack, .. } => Some(stack),
            Self::AuraReturn { stack, .. } => stack.as_mut(),
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

#[derive(Debug, Clone)]
pub struct PendingPlayerDiscardChoice {
    pub player: PlayerId,
    pub candidate_generations: Vec<(ObjectId, u64)>,
    pub required: u32,
}

/// Frozen hidden-hand choices for one simultaneous player-set discard action (CR 101.4).
/// `selections` is index-aligned with the completed prefix of `choices`; no selected identity is
/// published or moved until every required player has answered.
#[derive(Debug, Clone)]
pub struct PendingPlayerSetDiscard {
    pub choices: Vec<PendingPlayerDiscardChoice>,
    pub current: usize,
    pub selections: Vec<Vec<ObjectId>>,
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
pub enum EntryReplacementEffectId {
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
pub struct BattlefieldEntryEvent {
    pub object_id: ObjectId,
    /// CR 616.1 decider: current controller, or owner when the object has no controller.
    pub deciding_player: PlayerId,
    pub destination_controller: PlayerId,
    /// CR 310.11a: a Siege's controller chooses an opponent as protector while it enters.
    /// The choice is parked before zone commitment and installed atomically with entry.
    pub battle_protector: Option<PlayerId>,
    pub face_index: usize,
    /// The door selected while casting a Room permanent spell. Other battlefield-entry paths
    /// carry `None`; entering as a copy suppresses the designation at commitment.
    pub unlock_room_door: Option<usize>,
    /// X chosen for the entering permanent spell. Non-spell entry paths carry zero.
    pub chosen_x: u32,
    pub cast_cost_receipts: Vec<CastCostReceipt>,
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
pub struct TokenBattlefieldEntry {
    pub event: BattlefieldEntryEvent,
    pub created: TokenCreated,
}

#[derive(Debug, Clone)]
pub(crate) struct AttackingTokenBatch {
    pub defenders: Vec<tricerules_proto::ruled::v1::CombatDefenderOption>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingTokenEntryBatch {
    pub current_created: TokenCreated,
    pub ready: Vec<TokenBattlefieldEntry>,
    pub remaining: Vec<TokenBattlefieldEntry>,
    pub logs: Vec<String>,
    pub attacking: Option<AttackingTokenBatch>,
    pub delayed_sacrifice: Option<DelayedTokenSacrificeTiming>,
}

/// One simultaneous reanimation instruction, prepared fully before any member moves.
/// Replacement choices for Zombify/Reanimate use the same continuation as larger cohorts.
#[derive(Debug, Clone)]
pub(crate) struct PendingZoneEntryBatch {
    pub ready: Vec<BattlefieldEntryEvent>,
    pub remaining: Vec<BattlefieldEntryEvent>,
    pub generations: Vec<(ObjectId, u64)>,
    pub spell_label: String,
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
    ObserverReturn {
        owner: PlayerId,
        object_label: String,
        attached_to: Option<AttachmentRecipient>,
        resume_original_stack: bool,
    },
    LibrarySearch {
        owner: PlayerId,
        card_label: String,
        remaining_object_ids: Vec<ObjectId>,
        tapped: bool,
        shuffle: bool,
    },
    ManifestDread {
        owner: PlayerId,
        other_object_id: Option<ObjectId>,
        chosen_library_position: u32,
    },
    TokenBatch(Box<PendingTokenEntryBatch>),
    ZoneEntryBatch(Box<PendingZoneEntryBatch>),
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
    /// Stable authored identity captured when the positional command coordinate is accepted.
    pub mode_id: ModeId,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastCostObjectReceipt {
    RevealedHand {
        object_id: ObjectId,
        zone_change_generation: u64,
        card_id: String,
        card_name: String,
    },
    ChosenPermanent {
        object_id: ObjectId,
        zone_change_generation: u64,
        card_id: String,
        card_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastCostReceipt {
    pub group_index: u32,
    pub option_index: u32,
    pub group_id: Option<ChoiceId>,
    pub option_id: Option<ChoiceId>,
    pub label: String,
    pub object: Option<CastCostObjectReceipt>,
}

/// The announced procedure used to cast a spell. This is distinct from its source zone: more
/// than one alternative method may legally cast the same physical graveyard object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpellCastMethod {
    #[default]
    Normal,
    Flashback,
    Harmonize,
    SiegeDefeat,
    Warp,
    Permission,
}

impl SpellCastMethod {
    /// CR 702.34a / 702.180a: these methods replace every destination as the spell leaves the
    /// stack, including resolution into what would otherwise be a permanent.
    pub fn exiles_on_leave_stack(self) -> bool {
        matches!(self, Self::Flashback | Self::Harmonize)
    }

    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::Normal => None,
            Self::Flashback => Some("Flashback"),
            Self::Harmonize => Some("Harmonize"),
            Self::SiegeDefeat => Some("Siege defeat"),
            Self::Warp => Some("Warp"),
            Self::Permission => Some("Alternative cost"),
        }
    }
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
    /// CR 601.2b: the announced cast method, retained for method-specific stack-exit rules.
    pub cast_method: SpellCastMethod,
    /// CR 107.3b: the value chosen for `{X}` as this spell was cast. `0` for spells without an
    /// `{X}` pip (and for abilities). On the stack the spell's mana value is `fixed_mv + chosen_x`;
    /// at resolution this feeds [`Amount::X`](tricerules_cards::Amount) effect amounts.
    pub chosen_x: u32,
    /// Atomic modal choices in printed order. Empty for nonmodal spells and abilities.
    pub chosen_modes: Vec<ChosenMode>,
    /// CR 601.2b / 607.2i: stable announced additional-cost choices. Resolution and spell copies
    /// consume this receipt instead of rechecking the object used to pay.
    pub cast_cost_receipts: Vec<CastCostReceipt>,
    /// CR 601.2i / 608.2i: automatic public conditions captured for this actual cast, in face
    /// declaration order. Retained while parked, but not copied under CR 707.10: copies were
    /// never cast. Empty for abilities and directly created spell copies.
    pub cast_condition_results: Vec<bool>,
    /// The actual committed cast, not a copiable choice (Magebane Lizard, Thunder Salvo).
    /// Physical spells use their stack-entry generation; virtual cast copies use their unique ID.
    pub cast_occurrence: Option<StackObjectRef>,
    /// Exact card objects used to pay this spell or ability's costs. Copies retain the original
    /// cohort under CR 707.10.
    pub(crate) payment_result: CardResultCohort,
    /// Resolution branches already answered, keyed by their index in the original effect list.
    /// `None` records an optional decline; `Some(i)` records the chosen authored branch.
    pub resolution_branch_choices: BTreeMap<u32, Option<usize>>,
    pub blight_receipts: Vec<BlightReceipt>,
    /// Event-time player and object identity for triggered abilities. This includes an affected
    /// player (Howling Mine), an observed object distinct from CR 115 targets, and attack
    /// participants. Empty for spells and activated abilities. Trigger context never changes who
    /// controls the stack item.
    pub trigger_context: TriggerContext,
}

/// Presentation-only metadata for a live stack item. It is keyed separately so the mechanical
/// stack representation and its copy/resolve semantics remain unchanged.
#[derive(Debug, Clone, Default)]
pub struct StackPresentation {
    pub primary: Option<tricerules_proto::ruled::v1::PresentationRef>,
    pub chosen_modes: Vec<tricerules_proto::ruled::v1::PresentationRef>,
    pub chosen_cast_costs: Vec<tricerules_proto::ruled::v1::PresentationRef>,
}

impl StackItem {
    pub fn cast_cost_condition_matches(&self, condition: &CastCostReceiptCondition) -> bool {
        self.cast_cost_receipts.iter().any(|receipt| {
            receipt.group_id.as_ref() == Some(&condition.group_id)
                && receipt.option_id.as_ref() == Some(&condition.option_id)
        }) == condition.expected_selected
    }
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
    /// Present only for a granted triggered ability. Retained when unrelated effects expire;
    /// static grants use authored provenance, resolving grants a deterministic creation ID.
    pub trigger_grant_origin: Option<TriggerAbilityOrigin>,
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

/// The stack-bound payload of a one-shot delayed triggered ability (CR 603.7).
#[derive(Debug, Clone)]
pub struct DelayedTriggerPayload {
    /// Creating source, distinct from the watched object (Earthbend, delayed token sacrifice).
    pub source: TriggerObjectRef,
    pub controller: PlayerId,
    pub card_id: String,
    pub card_name: String,
    pub source_face_index: usize,
    /// Stable display identity captured while the creating stack item still exists.
    pub presentation: Option<tricerules_proto::ruled::v1::PresentationRef>,
    pub ability: TriggeredAbilityDef,
}

/// A closed set of event patterns observed after their underlying state transition commits.
/// Object events compare both ObjectId and zone-change generation (CR 400.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventObserverMatcher {
    AtBeginningOfNextEndStep,
    AtBeginningOfControllerNextTurnEndStep {
        controller: PlayerId,
        created_turn_instance: u64,
        target_turn_instance: Option<u64>,
    },
    WhenWatchedObjectDiesThisTurn,
    WhenWatchedObjectDiesOrIsExiled,
    WhenWatchedObjectLeavesBattlefield,
    WhenControllerLosesControlOf,
}

/// Work performed by a matching one-shot observer. Delayed triggers use the normal trigger/APNAP
/// queue; paired one-shot effects (CR 610.3) enqueue immediate work that is completed before the
/// engine advances to the next resolving instruction or grants priority.
#[derive(Debug, Clone)]
pub enum EventObserverPayload {
    StageDelayedTrigger(Box<DelayedTriggerPayload>),
    ReturnExiledObject { exiled: TriggerObjectRef },
}

#[derive(Debug, Clone)]
pub struct ActiveEventObserver {
    pub watched: TriggerObjectRef,
    pub matcher: EventObserverMatcher,
    pub payload: EventObserverPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObservedGameEvent {
    TurnBegan {
        active_player: PlayerId,
        turn_instance: u64,
    },
    BeginningOfEndStep {
        active_player: PlayerId,
        turn_instance: u64,
    },
    TurnEnded {
        active_player: PlayerId,
        turn_instance: u64,
    },
    Dies(TriggerObjectRef),
    LeavesBattlefield(TriggerObjectRef),
    /// Committed departure after replacement, used by Earthbend from spells and abilities.
    BattlefieldDeparture {
        object: TriggerObjectRef,
        destination: Zone,
        was_creature: bool,
    },
    ControllerChanged {
        object: TriggerObjectRef,
        old_controller: PlayerId,
        new_controller: Option<PlayerId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImmediateObserverAction {
    ReturnExiledObject { exiled: TriggerObjectRef },
}

/// During combat, after attack/block declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatDefenderTarget {
    Player(PlayerId),
    Permanent(TriggerObjectRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatAttackAssignment {
    pub attacker: TriggerObjectRef,
    pub defender: CombatDefenderTarget,
    pub defending_player: PlayerId,
}

#[derive(Debug, Clone)]
pub struct CombatState {
    pub attacking: Vec<ObjectId>,
    /// One generation-bound defender designation for every attacking creature (CR 508.1b).
    pub attack_assignments: HashMap<ObjectId, CombatAttackAssignment>,
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
    /// Actual positive changes, retained separately even when the net life change is zero.
    pub life_gained: u64,
    pub life_lost: u64,
    pub spells_cast: u32,
    pub crimes_committed: u32,
    /// CR 700.14: actual mana spent casting spells, including before an Expend watcher entered.
    pub mana_spent_casting_spells: u64,
    pub cards_drawn: u32,
    pub attacked: bool,
}

/// Event-time public characteristics for one committed attacker or battlefield entrant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnObjectFact {
    pub object_id: ObjectId,
    pub zone_change_generation: u64,
    pub controller: PlayerId,
    pub owner: PlayerId,
    pub is_token: bool,
    pub types: Vec<String>,
    pub all_creature_types: bool,
    pub keywords: Vec<Keyword>,
    pub power: Option<u32>,
}

impl TurnObjectFact {
    pub fn has_type(&self, card_type: &str) -> bool {
        self.types.iter().any(|value| value == card_type)
            || (self.all_creature_types && is_creature_type(card_type))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TurnRecord {
    pub nonland_permanent_left_battlefield: bool,
    pub spells_cast: u32,
    pub spell_casts: Vec<SpellCastFact>,
    pub creatures_died: u32,
    pub permanent_cards_entered_graveyard: Vec<PermanentHistoryFact>,
    pub permanents_sacrificed: Vec<PermanentHistoryFact>,
    pub by_player: BTreeMap<PlayerId, PlayerTurnRecord>,
    pub declared_attackers: Vec<TurnObjectFact>,
    pub permanents_entered: Vec<TurnObjectFact>,
    pub damaged_objects: Vec<(ObjectId, u64)>,
}

/// One committed occurrence, retained independently of the object's subsequent zone or types.
/// `player` is the graveyard owner for an entry and the acting controller for a sacrifice.
/// Entry types describe the destination card; sacrifice types describe the pre-move permanent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermanentHistoryFact {
    pub object_id: ObjectId,
    pub zone_change_generation: u64,
    pub player: PlayerId,
    pub types: Vec<String>,
}

/// Internal CR 601.2i history, shared by Magebane Lizard and Thunder Salvo. Characteristics
/// belong to the cast face at the event, not the card's later zone or face.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellCastFact {
    pub cast_method: SpellCastMethod,
    pub occurrence: StackObjectRef,
    pub caster: PlayerId,
    pub origin: Zone,
    pub face_index: usize,
    pub types: Vec<String>,
    pub all_creature_types: bool,
    pub mana_value: u32,
    pub matched_card_types: Vec<CardTypeFilter>,
    /// Deduplicated derived types of generation-matching battlefield permanents targeted when the
    /// spell became cast. No target identities are retained in turn history.
    pub targeted_permanent_types: Vec<PermanentTypeFilter>,
    pub ordinal: u32,
}

impl TurnRecord {
    pub fn void_holds(&self) -> bool {
        self.nonland_permanent_left_battlefield
            || self
                .spell_casts
                .iter()
                .any(|cast| cast.cast_method == SpellCastMethod::Warp)
    }

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
    /// Complete departure counter bags, keyed by the old incarnation (Drone / Ozolith).
    pub(crate) last_known_counters_by_generation:
        HashMap<(ObjectId, u64), BTreeMap<CounterKind, u32>>,
    /// Signed departure P/T for source-relative quantities (CR 608.2h).
    pub(crate) last_known_pt_by_generation: HashMap<(ObjectId, u64), (Option<i64>, Option<i64>)>,
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
    /// CR 310.11a: public protector chosen for each battlefield Siege. The zone-change funnel
    /// removes this mapping so a returned Battle must choose again.
    pub battle_protectors: HashMap<ObjectId, PlayerId>,
    pub stack: Vec<StackItem>,
    pub stack_presentations: HashMap<ObjectId, StackPresentation>,
    /// Index into players for who holds priority
    pub priority_idx: usize,
    pub active_player_idx: usize,
    pub turn_step: TurnStep,
    pub turn: u32,
    /// Monotonic across every player's turn; unlike `turn`, this never repeats across seats.
    pub turn_instance: u64,
    pub next_object_id: ObjectId,
    pub command_index: u64,
    /// Consecutive priority passes; reset when a spell/ability is added to stack
    pub passes_since_stack_change: u32,
    /// Number of lands played from hand this turn; compared against max (1 + extra land plays).
    pub lands_played_this_turn: u32,
    /// Successful limited activations in the current turn, keyed to full object and ability
    /// identity. Control changes do not affect the key (CR 602.5b).
    pub activation_uses_this_turn: HashMap<ActivationUseKey, u32>,
    /// Successful once-per-object activations. Old-generation entries are inert after a zone
    /// change and remain deterministic replay evidence for the lifetime of the game.
    pub activation_uses_per_object: HashMap<PersistentActivationUseKey, u32>,
    /// Persistent "triggers only once" usage. Generation-aware keys make leave-and-return a
    /// fresh object without copying or resetting usage on control changes or turn boundaries.
    pub triggered_once: HashSet<TriggerUseKey>,
    /// Printed trigger caps for the current actual turn instance, independent of active seat.
    /// Including the instance also makes direct deterministic turn-boundary fixtures safe.
    pub trigger_uses_this_turn: HashMap<(u64, TriggerUseKey), u32>,
    pub next_trigger_grant_id: u64,
    /// Internal committed tap instruction identity; shared by simultaneous transitions only.
    pub next_tap_action_id: u64,
    pub active_exile_play_permissions: Vec<ActiveExilePlayPermission>,
    pub next_exile_play_permission_group_id: u64,
    pub turn_history: TurnHistory,
    /// A custom resolution may move its physical card early. Only its completed resolution
    /// commits this generation-bound receipt to rules history; it is never a published field.
    pub(crate) deferred_graveyard_entry: Option<(StackObjectRef, PermanentHistoryFact)>,
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
    /// Generation-bound one-shot event observers. Both delayed triggers and paired one-shot
    /// effects use this closed dispatcher so object identity and event matching cannot drift.
    pub active_event_observers: Vec<ActiveEventObserver>,
    /// Battlefield incarnations that entered after being cast for their Warp cost. The public
    /// annotation is generation-bound so blink, bounce, and recast create an ordinary permanent.
    pub(crate) warped_permanent_incarnations: HashSet<(ObjectId, u64)>,
    /// Exact multi-object contexts keyed by the delayed trigger's primary observed object.
    /// Mobilize keeps its whole token cohort here without making ubiquitous TriggerContext
    /// values heap-owning or reconstructing identity from card names.
    pub observed_object_cohorts: HashMap<(ObjectId, u64), Vec<TriggerObjectRef>>,
    /// Immediate observer work discovered by low-level state transitions. The engine drains this
    /// before the next resolving instruction or priority boundary.
    pub(crate) pending_immediate_observer_actions: Vec<ImmediateObserverAction>,
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
    pub mana_restriction_presentations: Vec<Option<tricerules_proto::ruled::v1::PresentationRef>>,
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
    pub(crate) fn allocate_trigger_grant_origin(&mut self) -> TriggerAbilityOrigin {
        let origin = TriggerAbilityOrigin::ResolvingGrant(self.next_trigger_grant_id);
        self.next_trigger_grant_id += 1;
        origin
    }

    /// Install a grant with its stable identity. Static grants supply their authored origin;
    /// each resolving grant gets a fresh occurrence, including two grants in the same command.
    pub fn add_triggered_ability_grant(&mut self, mut effect: ContinuousEffect) {
        assert!(matches!(
            effect.kind,
            ContinuousEffectKind::GrantTriggeredAbility(_)
        ));
        if effect.trigger_grant_origin.is_none() {
            effect.trigger_grant_origin = Some(self.allocate_trigger_grant_origin());
        }
        self.continuous_effects.push(effect);
    }

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

    pub(crate) fn dispatch_event_observers(
        &mut self,
        event: ObservedGameEvent,
    ) -> Vec<(TriggerObjectRef, DelayedTriggerPayload)> {
        let mut waiting = std::mem::take(&mut self.active_event_observers);
        let mut delayed = Vec::new();
        for mut observer in waiting.drain(..) {
            let watched = observer.watched;
            let identity_matches = |observed: TriggerObjectRef| {
                observed.object_id == watched.object_id
                    && observed.zone_change_generation == watched.zone_change_generation
            };
            let mut expired = false;
            let matched = match (&mut observer.matcher, event) {
                (
                    EventObserverMatcher::WhenWatchedObjectDiesOrIsExiled,
                    ObservedGameEvent::BattlefieldDeparture {
                        object,
                        destination,
                        was_creature,
                    },
                ) if identity_matches(object) => {
                    expired = destination != Zone::Exile
                        && !(destination == Zone::Graveyard && was_creature);
                    !expired
                }
                (
                    EventObserverMatcher::AtBeginningOfNextEndStep,
                    ObservedGameEvent::BeginningOfEndStep { .. },
                ) => true,
                (
                    EventObserverMatcher::AtBeginningOfControllerNextTurnEndStep {
                        controller,
                        created_turn_instance,
                        target_turn_instance,
                    },
                    ObservedGameEvent::TurnBegan {
                        active_player,
                        turn_instance,
                    },
                ) if active_player == *controller
                    && turn_instance > *created_turn_instance
                    && target_turn_instance.is_none() =>
                {
                    *target_turn_instance = Some(turn_instance);
                    false
                }
                (
                    EventObserverMatcher::AtBeginningOfControllerNextTurnEndStep {
                        controller,
                        target_turn_instance: Some(target_turn_instance),
                        ..
                    },
                    ObservedGameEvent::BeginningOfEndStep {
                        active_player,
                        turn_instance,
                    },
                ) => active_player == *controller && turn_instance == *target_turn_instance,
                (
                    EventObserverMatcher::AtBeginningOfControllerNextTurnEndStep {
                        target_turn_instance: Some(target_turn_instance),
                        ..
                    },
                    ObservedGameEvent::TurnEnded { turn_instance, .. },
                ) if turn_instance == *target_turn_instance => {
                    expired = true;
                    false
                }
                (
                    EventObserverMatcher::WhenWatchedObjectDiesThisTurn,
                    ObservedGameEvent::Dies(observed),
                )
                | (
                    EventObserverMatcher::WhenWatchedObjectLeavesBattlefield,
                    ObservedGameEvent::LeavesBattlefield(observed),
                ) => identity_matches(observed),
                (
                    EventObserverMatcher::WhenControllerLosesControlOf,
                    ObservedGameEvent::ControllerChanged {
                        object,
                        old_controller,
                        new_controller,
                    },
                ) => {
                    identity_matches(object)
                        && old_controller == observer.watched.controller_at_event
                        && new_controller != Some(observer.watched.controller_at_event)
                }
                _ => false,
            };
            if expired {
                continue;
            }
            if !matched {
                self.active_event_observers.push(observer);
                continue;
            }
            match observer.payload {
                EventObserverPayload::StageDelayedTrigger(payload) => {
                    delayed.push((watched, *payload));
                }
                EventObserverPayload::ReturnExiledObject { exiled } => {
                    self.pending_immediate_observer_actions
                        .push(ImmediateObserverAction::ReturnExiledObject { exiled });
                }
            }
        }
        delayed
    }

    pub(crate) fn stage_delayed_batch(
        &mut self,
        delayed: Vec<(TriggerObjectRef, DelayedTriggerPayload)>,
    ) {
        let mut triggers = delayed
            .into_iter()
            .map(|(watched, delayed)| {
                let object_id = self.next_object_id;
                self.next_object_id += 1;
                let ability_text = delayed.ability.fallback_text(&delayed.card_name);
                StagedTrigger {
                    object_id,
                    source_permanent_id: delayed.source.object_id,
                    source_face_index: delayed.source_face_index,
                    source_zone_change: delayed.source.zone_change_generation,
                    source_face_change: 0,
                    card_id: delayed.card_id,
                    card_name: delayed.card_name,
                    controller: delayed.controller,
                    ability_index: 0,
                    ability_text,
                    presentation: delayed.presentation,
                    trigger_context: TriggerContext {
                        observed_object: Some(watched),
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

#[cfg(test)]
mod event_observer_tests {
    use super::*;
    use crate::engine::GameEngine;

    fn watched() -> TriggerObjectRef {
        TriggerObjectRef {
            object_id: 161,
            zone_change_generation: 2,
            controller_at_event: 0,
        }
    }

    fn controller_next_turn_observer(created_turn_instance: u64) -> ActiveEventObserver {
        ActiveEventObserver {
            watched: watched(),
            matcher: EventObserverMatcher::AtBeginningOfControllerNextTurnEndStep {
                controller: 0,
                created_turn_instance,
                target_turn_instance: None,
            },
            payload: EventObserverPayload::ReturnExiledObject { exiled: watched() },
        }
    }

    #[test]
    fn controller_next_turn_observer_arms_on_the_next_actual_turn() {
        let mut engine = GameEngine::new(161_010, &[0, 1], 20, None, true).expect("new");
        engine
            .state
            .active_event_observers
            .push(controller_next_turn_observer(7));

        engine
            .state
            .dispatch_event_observers(ObservedGameEvent::TurnBegan {
                active_player: 1,
                turn_instance: 8,
            });
        assert!(matches!(
            engine.state.active_event_observers[0].matcher,
            EventObserverMatcher::AtBeginningOfControllerNextTurnEndStep {
                target_turn_instance: None,
                ..
            }
        ));

        engine
            .state
            .dispatch_event_observers(ObservedGameEvent::TurnBegan {
                active_player: 0,
                turn_instance: 9,
            });
        assert!(matches!(
            engine.state.active_event_observers[0].matcher,
            EventObserverMatcher::AtBeginningOfControllerNextTurnEndStep {
                target_turn_instance: Some(9),
                ..
            }
        ));
        engine
            .state
            .dispatch_event_observers(ObservedGameEvent::BeginningOfEndStep {
                active_player: 0,
                turn_instance: 9,
            });
        assert!(engine.state.active_event_observers.is_empty());
        assert_eq!(engine.state.pending_immediate_observer_actions.len(), 1);
    }

    #[test]
    fn controller_next_turn_observer_expires_if_its_armed_turn_has_no_end_step() {
        let mut engine = GameEngine::new(161_011, &[0, 1], 20, None, true).expect("new");
        engine
            .state
            .active_event_observers
            .push(controller_next_turn_observer(12));
        engine
            .state
            .dispatch_event_observers(ObservedGameEvent::TurnBegan {
                active_player: 0,
                turn_instance: 13,
            });
        engine
            .state
            .dispatch_event_observers(ObservedGameEvent::TurnEnded {
                active_player: 0,
                turn_instance: 13,
            });

        assert!(engine.state.active_event_observers.is_empty());
        assert!(engine.state.pending_immediate_observer_actions.is_empty());
    }
}
