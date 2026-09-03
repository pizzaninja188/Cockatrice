//! Spell and continuous-effect vocabulary plus shared effect parameters.

use super::{
    ActivatedAbilityDef, Amount, BasePowerToughnessValue, CardTypeFilter, CastCostReceiptCondition,
    Color, ConditionObjectRef, CountExpression, CreatureScopeFilter, DamageDivision, EventZone,
    GameCondition, GraveyardDestination, GraveyardFilter, Keyword, LifeAmount, PermanentTypeFilter,
    PowerComparison, PowerToughnessCharacteristic, ProtectionQuality, ReflexiveTriggeredAbilityDef,
    SpecialActionKind, StackSpellFilter, TargetController, TargetFilter, TargetKind, TargetRole,
    TriggerCondition, TriggeredAbilityDef, TypeLineAddition, TypeLineReplacement,
};
#[cfg(test)]
use super::{
    BattlefieldAggregate, BattlefieldCreatureCountFilter, BattlefieldPermanentFilter,
    CreatureEventFilter, GraveyardAggregate, PermanentEventFilter,
};
use crate::{choice_fallback, AbilityPresentation, ChoiceId, ManaCost};
use serde::{Deserialize, Serialize};

fn default_one() -> u32 {
    1
}
/// A kind of counter that can sit on a permanent (CR 122.1). `Ord` is required so [`crate`]
/// consumers can store counters in a `BTreeMap` for deterministic iteration/serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CounterKind {
    /// CR 122 +1/+1 counter — adds 1 to power and toughness each (layer 7d).
    PlusOnePlusOne,
    /// CR 122 -1/-1 counter — subtracts 1 from power and toughness each (layer 7d).
    MinusOneMinusOne,
    /// CR 122.1b / 613.1f: grants the carried keyword while at least one remains.
    Keyword(Keyword),
    /// CR 122.1d: replaces an untap event by removing one stun counter.
    Stun,
    /// CR 306.5: loyalty counters on planeswalkers.
    Loyalty,
    /// CR 310.4: defense counters on battles.
    Defense,
    /// CR 714: lore counters advance Saga chapter abilities.
    Lore,
    /// CR 702.184: charge counters track Station progress on Spacecraft.
    Charge,
}

impl CounterKind {
    /// Short human-readable label for client display (e.g. in card annotations).
    /// Matches the conventional MTG counter naming ("+1/+1", "-1/-1").
    pub fn label(self) -> String {
        match self {
            CounterKind::PlusOnePlusOne => "+1/+1".into(),
            CounterKind::MinusOneMinusOne => "-1/-1".into(),
            CounterKind::Keyword(keyword) => keyword.as_str().to_ascii_lowercase(),
            CounterKind::Stun => "stun".into(),
            CounterKind::Loyalty => "loyalty".into(),
            CounterKind::Defense => "defense".into(),
            CounterKind::Lore => "lore".into(),
            CounterKind::Charge => "charge".into(),
        }
    }

    pub fn validate(self) -> Result<(), String> {
        match self {
            CounterKind::Keyword(keyword) if !keyword.can_be_keyword_counter() => Err(format!(
                "{} is not a legal keyword-counter kind under CR 122.1b",
                keyword.as_str()
            )),
            _ => Ok(()),
        }
    }
}

/// Counters applied as part of a battlefield-entry event. Keeping these on the proposed entry
/// composes with replacement effects before the permanent exists on the battlefield (CR 122.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterPlacement {
    pub counter: CounterKind,
    pub count: u32,
}

/// Departure LKI used by Dockworker Drone (Source) and The Ozolith (TriggerObject).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CounterSnapshotSource {
    Source,
    TriggerObject,
}

/// Where an effect is being resolved from. Controls validation that depends on context —
/// e.g. [`EffectSubject::Source`] is only meaningful for an ability bound to a source permanent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectContext {
    /// A spell's `spell_effect` list (no source permanent to self-reference).
    Spell,
    /// An activated or triggered ability bound to a source permanent.
    Ability,
}

/// The permanent affected by an effect that can either refer to its own source or target a
/// chosen permanent. `Source` is auto-bound and does not target in the CR 115 sense;
/// `Chosen` carries the filter for a genuine chosen target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectSubject {
    Source,
    /// The object the source Aura or Equipment is currently attached to. This is an untargeted
    /// rules reference ("enchanted creature" / "equipped creature"), not a CR 115 target.
    AttachedObject,
    /// The distinct permanent named by the trigger event. This is an untargeted rules reference
    /// (for example, the blocking creature affected by flanking), not a CR 115 target.
    TriggerObject,
    /// The exact generation-bound permanent selected by the immediately preceding
    /// `ChoosePermanents(min: 1, max: 1)` instruction. This is an untargeted CR 608 choice.
    PreviousEffectObject,
    Chosen(Box<TargetFilter>),
}

/// Additional legality applied to a public battlefield-permanent choice. The ordinary
/// [`TargetFilter`] still owns characteristics and controller scope; constraints compose facts
/// involving another object already bound to the resolving stack item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermanentChoiceConstraint {
    /// CR 301.5 / 608.2d / 701.3: the chosen permanent must be an Equipment that can currently
    /// attach to the referenced creature. Vow to Erebor uses this to avoid offering impossible
    /// resolution choices without turning the Equipment into a target.
    EquipmentAttachableTo { recipient: ConditionObjectRef },
}

/// A battlefield attachment subtype that an effect may enumerate through the authoritative
/// attachment-recipient relation. Aura and Equipment are deliberately distinct: both
/// attach to another game entity, but they have different legality and state-based actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AttachmentKind {
    Aura,
    Equipment,
}

/// OR-combined attachment kinds for effects that act on the current attachments of one object.
/// Turn to Slag selects Equipment; Flickerform and Rhuk, Hexgold Nabber establish reuse across
/// Aura and Equipment cohorts. Empty and duplicate filters are rejected at registry load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentFilter {
    pub kinds: Vec<AttachmentKind>,
}

impl AttachmentFilter {
    fn validate(&self) -> Result<(), String> {
        if self.kinds.is_empty() {
            return Err("attachment filter requires at least one kind".into());
        }
        if self
            .kinds
            .iter()
            .enumerate()
            .any(|(index, kind)| self.kinds[..index].contains(kind))
        {
            return Err("attachment filter cannot contain duplicate kinds".into());
        }
        Ok(())
    }
}

/// The quantity an affine [`SpellEffectKind::PumpTarget`] bonus scales from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PtScaleBasis {
    /// Resolve an ordinary nonnegative effect amount. Growth Cycle and Lavakin Brawler are the
    /// first spell and triggered-ability users.
    Amount(Amount),
    /// Snapshot the affected creature's current signed characteristic as the effect applies.
    /// Mightform Harmonizer and Unleash Fury use power; the signed form preserves CR 107.1b's
    /// exception for doubling a negative power or toughness.
    Subject(PowerToughnessCharacteristic),
}

/// An affine P/T bonus applied by [`SpellEffectKind::PumpTarget`]: resolve `basis`, multiply it by
/// the signed per-unit deltas, then add those results to the effect's fixed P/T bonus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PtScale {
    pub basis: PtScaleBasis,
    pub power_per_unit: i32,
    pub toughness_per_unit: i32,
}

impl PtScale {
    pub(crate) fn amount(&self) -> Option<&Amount> {
        match &self.basis {
            PtScaleBasis::Amount(amount) => Some(amount),
            PtScaleBasis::Subject(_) => None,
        }
    }

    fn validate_cast_snapshot_references(&self, count: usize) -> Result<(), String> {
        if let Some(amount) = self.amount() {
            amount.validate_cast_snapshot_references(count)?;
        }
        Ok(())
    }

    fn validate_effect(&self, context: EffectContext) -> Result<(), String> {
        if self.power_per_unit == 0 && self.toughness_per_unit == 0 {
            return Err("P/T scale requires a nonzero per-unit modifier".into());
        }
        if let Some(amount) = self.amount() {
            amount.validate_effect(context)?;
        }
        Ok(())
    }

    fn requires_triggering_spell_context(&self) -> bool {
        self.amount()
            .is_some_and(Amount::requires_triggering_spell_context)
    }
}

/// The two distinct CR face-change actions. Transform toggles an eligible double-faced
/// permanent; flip changes an unflipped Kamigawa flip permanent to its flipped status once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaceChangeAction {
    Transform,
    Flip,
}

impl Default for EffectSubject {
    fn default() -> Self {
        Self::Chosen(Box::new(TargetFilter::default_creature()))
    }
}

/// CR 509.1b: cumulative rule-changing restrictions, shared by Argothian Sprite, Verdant
/// Outrider, Rampaging Ceratops and the Faerie created by Into the Fae Court. Filters describe
/// creatures, not targets; shroud and hexproof do not participate in blocking legality.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CombatRestriction {
    #[serde(default)]
    pub cant_attack: bool,
    #[serde(default)]
    pub cant_block: bool,
    #[serde(default)]
    pub cant_be_blocked: bool,
    #[serde(default)]
    pub cant_be_blocked_by: Vec<TargetFilter>,
    #[serde(default)]
    pub cant_block_creatures_matching: Vec<TargetFilter>,
    /// Bounds apply only to nonempty blocker groups. Menace contributes a minimum of two.
    #[serde(default)]
    pub minimum_blockers: Option<u32>,
    #[serde(default)]
    pub maximum_blockers: Option<u32>,
}

impl CombatRestriction {
    /// Public descriptions consumed by the existing generic rules-annotation display.
    pub fn labels(&self) -> Vec<String> {
        let mut labels = Vec::new();
        if self.cant_attack {
            labels.push("Can't attack".into());
        }
        if self.cant_block {
            labels.push("Can't block".into());
        }
        if self.cant_be_blocked || self.maximum_blockers == Some(0) {
            labels.push("Can't be blocked".into());
        }
        for filter in &self.cant_be_blocked_by {
            labels.push(format!(
                "Can't be blocked by {}",
                combat_creature_description(filter)
            ));
        }
        for filter in &self.cant_block_creatures_matching {
            labels.push(format!(
                "Can't block {}",
                combat_creature_description(filter)
            ));
        }
        if let Some(minimum) = self.minimum_blockers.filter(|min| *min > 1) {
            labels.push(format!(
                "Can't be blocked except by {minimum} or more creatures"
            ));
        }
        if let Some(maximum) = self.maximum_blockers.filter(|max| *max > 0) {
            labels.push(format!(
                "Can't be blocked by more than {maximum} {}",
                if maximum == 1 {
                    "creature"
                } else {
                    "creatures"
                }
            ));
        }
        let mut unique = Vec::new();
        for label in labels {
            if !unique.contains(&label) {
                unique.push(label);
            }
        }
        unique
    }

    pub fn is_empty(&self) -> bool {
        !self.cant_attack
            && !self.cant_block
            && !self.cant_be_blocked
            && self.cant_be_blocked_by.is_empty()
            && self.cant_block_creatures_matching.is_empty()
            && self.minimum_blockers.is_none()
            && self.maximum_blockers.is_none()
    }

    pub fn combine(&mut self, other: &Self) {
        self.cant_attack |= other.cant_attack;
        self.cant_block |= other.cant_block;
        self.cant_be_blocked |= other.cant_be_blocked;
        self.cant_be_blocked_by
            .extend(other.cant_be_blocked_by.iter().cloned());
        self.cant_block_creatures_matching
            .extend(other.cant_block_creatures_matching.iter().cloned());
        self.minimum_blockers = self
            .minimum_blockers
            .into_iter()
            .chain(other.minimum_blockers)
            .max();
        self.maximum_blockers = self
            .maximum_blockers
            .into_iter()
            .chain(other.maximum_blockers)
            .min();
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.is_empty() {
            return Err("combat restriction requires at least one restriction".into());
        }
        if self.minimum_blockers == Some(0)
            || self
                .minimum_blockers
                .zip(self.maximum_blockers)
                .is_some_and(|(min, max)| min > max)
        {
            return Err("invalid authored blocker count bounds".into());
        }
        for filter in self
            .cant_be_blocked_by
            .iter()
            .chain(&self.cant_block_creatures_matching)
        {
            filter.validate_characteristic_constraints()?;
            if !filter.all_terminal_filters_match(|leaf| {
                leaf.kind == TargetKind::Creature
                    && leaf.controller == TargetController::Any
                    && leaf.excluded_objects.is_empty()
                    && leaf.combat_role.is_none()
                    && leaf.tapped.is_none()
            }) {
                return Err("combat predicates require creature characteristics without controller, source, tapped or combat-role selectors".into());
            }
        }
        Ok(())
    }
}

fn combat_creature_description(filter: &TargetFilter) -> String {
    if let Some(branches) = &filter.any_of {
        return branches
            .iter()
            .map(combat_creature_description)
            .collect::<Vec<_>>()
            .join(" or ");
    }
    let mut clauses = Vec::new();
    if !filter.permanent_types.is_empty() {
        clauses.push(format!(
            "with type {}",
            filter
                .permanent_types
                .iter()
                .map(|kind| format!("{kind:?}"))
                .collect::<Vec<_>>()
                .join(" or ")
        ));
    }
    for kind in &filter.excluded_permanent_types {
        clauses.push(format!("that aren't {kind:?}s").to_lowercase());
    }
    if let Some(color) = filter.is_color {
        clauses.push(format!("that are {color:?}").to_lowercase());
    }
    if let Some(color) = filter.not_color {
        clauses.push(format!("that aren't {color:?}").to_lowercase());
    }
    for subtype in &filter.required_subtypes {
        clauses.push(format!("with subtype {subtype}"));
    }
    for subtype in &filter.excluded_subtypes {
        clauses.push(format!("without subtype {subtype}"));
    }
    for keyword in &filter.required_keywords {
        clauses.push(format!("with {}", keyword.as_str().to_lowercase()));
    }
    for keyword in &filter.excluded_keywords {
        clauses.push(format!("without {}", keyword.as_str().to_lowercase()));
    }
    if let Some(comparison) = filter.power {
        let (value, bound) = match comparison {
            super::PowerComparison::AtLeast(value) => (value, "more"),
            super::PowerComparison::AtMost(value) => (value, "less"),
        };
        clauses.push(format!("with power {value} or {bound}"));
    }
    if clauses.is_empty() {
        "creatures".into()
    } else {
        format!("creatures {}", clauses.join(" and "))
    }
}

/// Which creature(s) receive a resolving [`CombatRestriction`]. `Matching` is deliberately
/// dynamic: rule-changing effects such as Destructive Tampering continuously re-evaluate which
/// creatures lack Flying, including creatures entering after the spell resolves (CR 611.2c).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatRestrictionScope {
    Source,
    Chosen(TargetFilter),
    Matching(TargetFilter),
}

/// Where a battlefield permanent is placed in its owner's library.
///
/// `Shuffle` means the permanent is moved first and the complete library is then shuffled.
/// Totally Lost and Griptide use `Top`; Deglamer and Unravel the Aether use `Shuffle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LibraryPlacement {
    Top,
    SecondFromTop,
    Bottom,
    Shuffle,
    /// The targeted object's owner makes a logged resolution-time choice. Uncharted Voyage and
    /// Riverwalk Technique share this owner-relative placement primitive.
    OwnerChoiceTopOrBottom,
    /// Lost Days: the targeted object's owner chooses second from the top or bottom.
    OwnerChoiceSecondFromTopOrBottom,
}

/// How cards left over from a bounded library look are placed on the bottom.
///
/// Brightwood Tracker uses `Random`; Commune with Nature uses `Chosen`. `Random` randomizes only
/// the looked-at cohort, not the complete library, so it is deliberately distinct from
/// [`LibraryPlacement::Shuffle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LibraryBottomOrder {
    Random,
    Chosen,
}

/// Rules meaning attached to a top-library partition.
///
/// `Surveil` is the CR 701.25 keyword action and therefore emits the corresponding completed
/// game event. `Look` covers nonkeyword instructions such as Gutless Plunderer that use the same
/// private partition machinery without counting as surveilling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LibraryPartitionKind {
    Surveil,
    Look,
}

/// Who chooses cards for a discard instruction.
///
/// CR 701.9b makes the affected player the default chooser. Coercion and Thoughtseize override
/// that default by instructing their controller to choose, while Hymn to Tourach uses a seeded
/// random choice. Keeping these as typed modes prevents hand visibility from being inferred by
/// the client or relay.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscardChooser {
    #[default]
    AffectedPlayer,
    Controller,
    Random,
}

/// Who is authorized to see a hidden-hand cohort while a controller-selected hand effect is
/// parked for its resolution choice. The default is fail-closed: cards such as Cracked Skull say
/// "look" and show the hand only to the chooser, while Coercion, Thoughtseize, and Aggressive
/// Negotiations explicitly reveal the hand to every player (CR 701.20).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandChoiceVisibility {
    #[default]
    PrivateLook,
    PublicReveal,
}

/// Written order for a draw/discard sequence whose discard may suspend resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawDiscardOrder {
    DrawThenDiscard,
    DiscardThenDraw,
}

/// A fixed protection quality or an engine-authored list chosen as the effect resolves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtectionGrant {
    Fixed(ProtectionQuality),
    Choose(Vec<ProtectionQuality>),
}

/// A single optional or mandatory cost offered while an effect resolves (CR 118.12, 608.2d/g).
/// The initial branch vocabulary deliberately has one cost per branch, which covers alternate
/// sacrifice/discard branches without introducing partial multi-cost payment state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionCost {
    /// Foggy Swamp Vinebender and Waterbending Lesson share the CR 701.67 payment operation.
    Waterbend(ManaCost),
    /// Dream Seizer and Blighted Blackthorn: all N counters on one controlled creature.
    Blight {
        count: u32,
    },
    /// A labeled mandatory/optional branch with no payment. Used by generic resolution choices
    /// whose consequence is encoded by the parent effect, such as choosing a protection quality.
    None,
    Mana(ManaCost),
    DiscardCard {
        #[serde(default)]
        filter: Option<CardTypeFilter>,
    },
    SacrificePermanent {
        filter: TargetFilter,
        /// Servant of the Stinger pays with its own incarnation; Crypt Lurker can pay with
        /// any matching permanent. This is a payment restriction, never a target.
        #[serde(default)]
        source_only: bool,
    },
    TapPermanents {
        count: u32,
        filter: TargetFilter,
        #[serde(default)]
        exclude_source: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionBranchDef {
    pub branch_id: ChoiceId,
    pub presentation: AbilityPresentation,
    /// Engine-derived wording for runtime-only synthetic choices. It cannot be authored in RON.
    #[serde(skip)]
    pub runtime_fallback: Option<String>,
    pub cost: ResolutionCost,
    #[serde(default)]
    pub requirement: ResolutionBranchRequirement,
    #[serde(default)]
    pub effects: Vec<SpellEffectKind>,
}

impl ResolutionBranchDef {
    pub fn fallback_label(&self) -> String {
        if let Some(fallback) = &self.runtime_fallback {
            return fallback.clone();
        }
        match &self.cost {
            ResolutionCost::Waterbend(cost) => format!("Waterbend {cost}"),
            ResolutionCost::Blight { count } => format!("Blight {count}"),
            ResolutionCost::Mana(cost) => format!("Pay {cost}"),
            ResolutionCost::DiscardCard { .. } => "Discard a card".into(),
            ResolutionCost::SacrificePermanent { .. } => "Sacrifice a permanent".into(),
            ResolutionCost::TapPermanents { count, .. } => {
                format!("Tap {count} permanent(s)")
            }
            ResolutionCost::None => choice_fallback("Choice", &self.branch_id),
        }
    }
}

/// Selects an engine-owned cohort of cards produced while paying for the resolving stack item or
/// by the immediately preceding primitive instruction. The cohort is never published; consumers
/// expose only their ordinary public result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardResultFilter {
    pub source: CardResultSource,
    pub action: CardResultAction,
    pub players: RelativePlayerSet,
    #[serde(default)]
    pub card_type: Option<CardTypeFilter>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardResultSource {
    Payment,
    PreviousEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardResultAction {
    Discard,
    /// A permanent that was actually destroyed, even when a replacement effect changes the
    /// destination. Indestructible and regeneration prevent this result (CR 701.7). Shredder's
    /// Technique and Make Yourself Useful consume this generic "destroyed this way" receipt.
    Destroy,
    Exile,
    Sacrifice,
    Mill,
    Tap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ResolutionBranchRequirement {
    #[default]
    Always,
    EffectsApplicable,
    /// The branch is live only when this public game-state predicate holds as the instruction
    /// resolves. Trade Route Envoy and Embermouth Sentinel use condition/fallback branches.
    GameCondition(GameCondition),
    /// A linked cast-time choice recorded on this spell's stack item (CR 607.2i). Kicker and
    /// behold effects remain true even if the object used for the choice later changes zones.
    CastCostReceipt(CastCostReceiptCondition),
    /// A typed result emitted by the immediately preceding primitive instruction. Divert
    /// Disaster distinguishes an actual soft-counter payment from a decline without inspecting
    /// mana pools or logs.
    PreviousResultReceipt(ResolutionReceiptCondition),
    /// Compare the size of an exact paid-or-moved card cohort. Grab the Prize reads its discard
    /// payment; Soul-Shackled Zombie and Fanatic of the Harrowing read a preceding instruction.
    CardResultCount {
        filter: CardResultFilter,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionReceiptCondition {
    CounterUnlessPaid { paid: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ResolutionBranchSelection {
    /// Present every live branch to the deciding player through the existing choice contract.
    #[default]
    PlayerChoice,
    /// Resolve the first live branch in authored order without publishing a player choice.
    FirstApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CastCostConditionalAmount {
    pub condition: CastCostReceiptCondition,
    pub if_selected: u32,
    pub otherwise: u32,
}

/// One independently filtered capacity in a heterogeneous hidden-zone search. Aang's Journey
/// uses a basic-land slot plus a kicked Shrine slot; the same graph represents searches such as
/// Gem of Becoming without exposing card-type inference to clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchSelectionSlot {
    pub slot_id: ChoiceId,
    pub presentation: AbilityPresentation,
    pub filter: ZoneCardFilter,
    #[serde(default)]
    pub enabled_by_cast_cost: Option<CastCostReceiptCondition>,
}

impl SearchSelectionSlot {
    pub fn fallback_label(&self) -> String {
        choice_fallback("Search choice", &self.slot_id)
    }
}

/// Timing for a delayed trigger that sacrifices the full post-replacement token cohort.
/// Mobilize uses the next end step; Kav Landseeker uses the end step of its controller's next
/// actual turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelayedTokenSacrificeTiming {
    NextEndStep,
    ControllerNextTurnEndStep,
}

/// The creature subtype named by an Amass instruction (CR 701.47). A closed enum keeps the
/// authored action aligned with a canonical token definition instead of accepting arbitrary
/// strings that might not have a matching 0/0 Army token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArmySubtype {
    Goblin,
    Zombie,
}

impl ArmySubtype {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Goblin => "Goblin",
            Self::Zombie => "Zombie",
        }
    }

    pub fn plural(self) -> &'static str {
        match self {
            Self::Goblin => "Goblins",
            Self::Zombie => "Zombies",
        }
    }

    pub fn token_id(self) -> &'static str {
        match self {
            Self::Goblin => "goblin_army_b_0_0",
            Self::Zombie => "zombie_army_b_0_0",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellEffectKind {
    /// Apply one ordinary non-suspending instruction only when a current engine-side condition
    /// holds. Target roles come solely from `effect`; `condition` never narrows initial legality.
    Conditional {
        condition: GameCondition,
        effect: Box<SpellEffectKind>,
    },
    /// Resolve one ordinary instruction only when the announced cast-cost receipt matches.
    ConditionalCastCost {
        condition: CastCostReceiptCondition,
        effect: Box<SpellEffectKind>,
    },
    /// CR 701.66: persistent land animation followed by a generation-bound delayed return.
    /// Rebellious Captives, Dai Li Indoctrination, and Badgermole share this action.
    Earthbend {
        count: Amount,
    },
    DamageTarget {
        amount: Amount,
        target: TargetFilter,
    },
    /// Create a turn-scoped CR 614 replacement for the exact targeted permanent generation:
    /// if it would be put into a graveyard from the battlefield, exile it instead. This is a
    /// separate instruction from damage because the replacement still exists when all damage is
    /// prevented. Cards: Lava Coil and Scorching Dragonfire.
    ExileIfWouldDieThisTurn {
        target: TargetFilter,
    },
    /// A chosen creature deals noncombat damage equal to its current power to another chosen
    /// creature. Both roles are targets, but the selected creature rather than the resolving
    /// spell is the damage source. Cards: Rabid Bite and Hunter's Edge.
    CreatureDealsDamageEqualToPower {
        source: TargetFilter,
        target: TargetFilter,
    },
    /// CR 701.14: two creatures deal noncombat damage equal to their current power to each
    /// other simultaneously. A chosen subject is a genuine CR 115 target; source, attachment,
    /// and trigger-object subjects are ability-bound references and do not target. This shared
    /// shape covers two-target spells (Prey Upon, Bushwhack) and source-bound creature abilities.
    Fight {
        first: EffectSubject,
        second: EffectSubject,
    },
    /// CR 120.3a: deal `amount` damage to a player, chosen by `who`. **Untargeted** — "that
    /// player" and "you" name a player without targeting it (CR 115.1), so this is deliberately
    /// absent from `spell_effect_kind_needs_target` and from [`Self::target_filters`], exactly
    /// like the neighbouring [`Self::LoseLife`].
    ///
    /// Damage, not life loss, and the distinction is load-bearing: a prevention shield consumes
    /// it (CR 615.1) and a future "whenever a source deals damage" trigger can observe it, while
    /// `TargetPlayerLosesLife` would be invisible to both — and would additionally target.
    ///
    /// Cards: Sulfuric Vortex and Tangle Wire (`AffectedPlayer`, "that player"), Serendib Efreet
    /// and Juzám Djinn (`Controller`, "to you"), Underworld Dreams, Ebony Owl Netsuke.
    ///
    /// `Amount` rather than a bare `u32` follows [`Self::DamageTarget`] — `DamageAll`'s `u32` is
    /// the known-narrow one (it cannot express Earthquake). Triggered abilities normally resolve
    /// X as 0; a self ETB trigger from an X-cost spell retains that spell's chosen X.
    DamagePlayer {
        amount: Amount,
        #[serde(default)]
        who: PlayerRecipient,
    },
    /// Untargeted damage to the player or planeswalker this source attacked. The attacked
    /// recipient is captured by the declaration trigger context; attacking a Battle yields no
    /// recipient. Cards: Scorch Spitter and Tectonic Giant-style attack-recipient effects.
    DamageAttackedPlayerOrPlaneswalker {
        amount: Amount,
    },
    /// Divide `amount` damage among any number of targets (CR 601.2d). Costs
    /// `extra_mana_per_target` additional generic mana per target beyond the first (Fireball = 1,
    /// Fire = 0). Target cardinality is declared by the sibling [`TargetingDef`](crate::TargetingDef).
    /// At cast time the player submits `(target, damage_amount)` pairs via `TargetRef`; the sum
    /// must equal the amount resolved from `x_value`. Covers Fireball (X divided unlimited) and Fire
    /// (fixed 2 divided among ≤ 2 targets). CR 608.2b: if some targets become illegal at
    /// resolution, damage is applied only to the remaining legal targets (partial fizzle).
    DamageTargets {
        amount: Amount,
        target: TargetFilter,
        #[serde(default)]
        division: DamageDivision,
        #[serde(default)]
        extra_mana_per_target: u32,
    },
    Draw {
        #[serde(default)]
        who: PlayerRecipient,
        count: Amount,
    },
    /// CR 121.2: the chosen player draws `count` cards. Jace Beleren and Ancestral Recall share
    /// this targeted form; untargeted draws continue to use [`Self::Draw`].
    TargetPlayerDraws {
        count: u32,
        target: TargetFilter,
    },
    /// CR 701.9: each affected player chooses and discards `count` cards without targeting.
    /// Player-set recipients make their hidden choices in APNAP order before the complete discard
    /// action is applied. Cards: Fanatic of the Harrowing, Burglar Rat, and Macabre Waltz.
    Discard {
        #[serde(default)]
        who: PlayerRecipient,
        count: u32,
    },
    /// Draw and discard as one resumable instruction. This is intentionally untargeted: `who`
    /// identifies the affected player and that player chooses from their private hand.
    ///
    /// Rousing Read and Teferi's Protege draw then discard. Keldon Raider optionally discards,
    /// then draws only if a card was actually discarded.
    DrawDiscard {
        #[serde(default)]
        who: PlayerRecipient,
        draw_count: u32,
        discard_count: u32,
        order: DrawDiscardOrder,
        #[serde(default)]
        optional: bool,
    },
    /// Present engine-authored branches as an in-resolution choice. The chosen branch's single
    /// cost is validated and paid atomically, then its effects run in authored order.
    ChooseResolutionBranch {
        #[serde(default)]
        chooser: PlayerRecipient,
        #[serde(default)]
        optional: bool,
        #[serde(default)]
        selection: ResolutionBranchSelection,
        branches: Vec<ResolutionBranchDef>,
    },
    /// Choose a bounded set of battlefield permanents during resolution without targeting.
    /// The result is engine-private and may be consumed by the immediately following effect.
    /// Final Showdown, Sakashima's Will, and Polymorphous Rush share this selection boundary.
    ChoosePermanents {
        #[serde(default)]
        chooser: PlayerRecipient,
        filter: TargetFilter,
        min: u32,
        max: u32,
        #[serde(default)]
        constraints: Vec<PermanentChoiceConstraint>,
    },
    /// Stage a reflexive triggered ability created by the immediately preceding paid branch.
    /// Sparktongue Dragon and Heart-Piercer Manticore share this CR 603.12 primitive.
    CreateReflexiveTrigger {
        ability: Box<ReflexiveTriggeredAbilityDef>,
    },
    /// CR 701.18: look at the top `count` cards of your library, put any number of them on the
    /// bottom of it in any order, and the rest back on top in any order. The cards never leave the
    /// library, so this fires no zone-change triggers.
    ///
    /// Resolution suspends for the player's decision — up to two interrupts, the second (ordering
    /// the cards kept on top) skipped when it would be a no-op. Because it suspends, any effect
    /// declared after it resumes via `PendingResolution::resume_effect_index`; `[Scry, Draw]` is
    /// Preordain and Opt.
    Scry {
        count: Amount,
    },
    /// Look at the top `count` cards, put a bounded number back on top in any order, and put the
    /// rest into the graveyard in any order. The player selects the graveyard cohort first; when
    /// two or more cards remain, resolution parks again for their top-library order.
    ///
    /// Surveil (CR 701.25) uses an unrestricted top cohort and emits a surveil event only after
    /// both choices complete. Gutless Plunderer uses `Look` with at most one card retained.
    LibraryPartition {
        count: u32,
        top_min: u32,
        top_max: Option<u32>,
        kind: LibraryPartitionKind,
    },
    /// CR 701.62: look at the top two cards, manifest one, then put the other into the
    /// graveyard. The choice is private, logged, and resumable through the engine's library
    /// picker. Bashful Beastie, Innocuous Rat, Manifest Dread, and Twist Reality.
    ManifestDread,
    /// Look at the top `count` cards, optionally reveal one matching `filter` and put it into the
    /// controller's hand, then put the rest on the bottom. All looked-at cards are displayed in
    /// the private card-image picker; the engine separately publishes which images match.
    ///
    /// Brightwood Tracker (`count: 4`, creature, random) and Commune with Nature (`count: 5`,
    /// creature, chosen) share this resumable primitive.
    LookChooseToHand {
        count: u32,
        filter: ZoneCardFilter,
        bottom_order: LibraryBottomOrder,
    },
    /// CR 701.7: destroy `subject`. Chosen subjects are CR 115 targets; source, attachment, and
    /// trigger-object subjects are untargeted rules references. Murder and Royal Assassin use
    /// chosen subjects, while Cracked Skull uses the damage event's trigger object.
    Destroy {
        #[serde(default = "default_destroy_subject")]
        subject: EffectSubject,
    },
    /// CR 701.17: sacrifice an untargeted subject. Source-bound triggers such as Command Bridge
    /// use this instead of destroy so indestructible and regeneration never apply.
    Sacrifice {
        subject: EffectSubject,
    },
    /// Destroy every current Aura and/or Equipment attached to one chosen permanent. The target
    /// is revalidated normally under CR 608.2b; the untargeted attachment cohort is determined
    /// once when this instruction is applied (CR 608.2h). Turn to Slag uses `Equipment`.
    DestroyAttached {
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
        attachments: AttachmentFilter,
    },
    /// Give +power/+toughness until end of turn to `subject`. The default is a chosen creature
    /// target (Giant Growth); `Source` auto-binds an ability to its source permanent.
    PumpTarget {
        power: i32,
        toughness: i32,
        #[serde(default)]
        scale: Option<PtScale>,
        #[serde(default)]
        subject: EffectSubject,
    },
    /// CR 611.2d / 613.4b: set one target permanent's base P/T until end of turn. `power` and
    /// `toughness` are independent so Galion can snapshot unequal source characteristics.
    SetBasePowerToughness {
        target: TargetFilter,
        power: BasePowerToughnessValue,
        toughness: BasePowerToughnessValue,
    },
    /// CR 701.19: tap `subject`. `Chosen` preserves ordinary permanent targeting.
    Tap {
        #[serde(default)]
        subject: EffectSubject,
    },
    /// CR 502.3 / 611.2a: the chosen permanent does not untap during its controller's next
    /// untap step. The restriction follows the permanent's current controller, is consumed by
    /// that untap step even if the permanent is already untapped, and does not survive a zone
    /// change. Crippling Chill and Frost Breath share this primitive.
    SkipNextUntap {
        target: TargetFilter,
    },
    /// Tap every creature controlled by the selected relative player set. This is untargeted and
    /// snapshots the battlefield as it resolves. Covers Cryptic Command and Tempest Caller.
    TapAllCreatures {
        players: RelativePlayerSet,
    },
    /// CR 701.20: untap `subject`. `Chosen` preserves ordinary permanent targeting for Seeker of
    /// Skybreak and Aphetto Alchemist; `Source` is the untargeted self-reference used by
    /// Sabertooth Mauler. An untapped chosen permanent is still legal and the effect is a no-op.
    Untap {
        #[serde(default)]
        subject: EffectSubject,
    },
    /// CR 701.20: untap every permanent matching `filter` controlled by `players`. Untargeted, and
    /// snapshots the battlefield as it resolves, like [`Self::TapAllCreatures`]. Controller scope
    /// lives in `players` rather than in the filter's `controller`, because untargeted mass
    /// selection goes through `battlefield_objects_matching`, which has no activating player to
    /// compare against. Covers Vitalize (`Controller` + creature) and Early Harvest / Turnabout
    /// (a player's lands).
    UntapAll {
        players: RelativePlayerSet,
        #[serde(default = "TargetFilter::default_creature")]
        filter: TargetFilter,
    },
    /// CR 611.2a / 613.1b: the resolving spell's controller gains control of target permanent
    /// until cleanup. Act of Treason and Threaten compose this with Untap and GrantKeywords.
    GainControlUntilEndOfTurn {
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
    },
    /// CR 701.6: counter target spell on the stack. `spell_filter` narrows which spells are legal
    /// targets by type and/or inclusive mana-value bounds. The default is unrestricted
    /// (Counterspell); creature and noncreature type filters cover Essence Scatter and Negate;
    /// exact and minimum mana values cover Spell Snare and Disdainful Stroke.
    /// `unless_controller_pays` parks resolution for an optional
    /// generic-mana payment by that spell's controller (Convolute, Mana Leak). The composed
    /// filter keeps type and mana-value restrictions on one reusable target path.
    CounterTargetSpell {
        #[serde(default)]
        spell_filter: StackSpellFilter,
        #[serde(default)]
        unless_controller_pays: Option<Amount>,
        /// Receipt-conditioned soft-counter amount. Dispelling Exhale uses `{4}` when a Dragon
        /// was beheld and `{2}` otherwise. Mutually exclusive with `unless_controller_pays`.
        #[serde(default)]
        unless_controller_pays_by_cast_cost: Option<CastCostConditionalAmount>,
    },
    /// CR 702.21: counter the exact spell or ability whose target-selection event created this
    /// trigger unless that object's controller pays `cost`. This is an untargeted event reference,
    /// not a CR 115 stack target. Cackling Prowler and Dirgur Island Dragon use mana; Spectral
    /// Snatcher uses a discard cost.
    CounterTriggeringStackObjectUnlessPays {
        cost: ResolutionCost,
    },
    /// CR 707.10: put `count` copies of target spell on the stack, each controlled by this
    /// spell's controller. A copy is **not cast** (no mana, no cast triggers, no storm count) and
    /// ceases to exist after it resolves (CR 707.10d). The copy uses the original's chosen modes,
    /// X, and targets; CR 707.10c lets the copy's controller choose new targets (deferred — copies
    /// keep the original's targets for now). `count` covers Twincast / Fork / Reverberate (1) and
    /// "copy it twice" effects without a new variant. `spell_filter` restricts the legal target
    /// the same way as [`Self::CounterTargetSpell`] — `card_type: Some(InstantOrSorcery)` for
    /// Twincast / Reverberate ("copy target instant or sorcery spell"); only spells (not
    /// abilities) qualify.
    CopyTargetSpell {
        #[serde(default = "one")]
        count: u32,
        #[serde(default)]
        spell_filter: StackSpellFilter,
    },
    /// CR 613.4 layer 7c: give every creature matching `filter` +power/+toughness until end of
    /// turn (the mass, one-shot sibling of [`Self::PumpTarget`]). Untargeted — `filter` selects
    /// the set the same way a static anthem does. Glorious Charge / Inspired Charge
    /// (`controller: YouControl`); attacking-creature pumps reuse the same filter machinery.
    PumpAll {
        #[serde(default)]
        filter: CreatureScopeFilter,
        power: i32,
        toughness: i32,
    },
    /// CR 303.4: the aura's "Enchant [type]" clause. Authored in `spell_effect` of every Aura
    /// enchantment — it is the sole effect that requires a target during casting, and at resolution
    /// it records the attachment (engine sets `attached_to` before processing this effect). The
    /// `target` filter mirrors the card's "Enchant [type]" line; default is any creature. Validated
    /// at registry load to reject player-kind filters (auras enchant permanents, CR 303.4a).
    AuraAttach {
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
    },
    /// CR 701.3 / 301.5: attach the exact Equipment source to a chosen permanent. Unlike Equip,
    /// this is an ordinary one-shot instruction and does not impose sorcery timing. Illvoi Light
    /// Jammer, Squire's Lightblade, Meltstrider's Gear, and Barbed Bloodletter share this ETB
    /// primitive.
    AttachSource {
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
    },
    /// CR 301.5 / 701.3: attach one independently selected Equipment to one selected creature.
    /// Swordsman, Sharp Scoundrel binds both subjects as targets; Vow to Erebor consumes a
    /// generation-bound non-target Equipment choice followed by its existing creature target.
    AttachEquipment {
        equipment: EffectSubject,
        creature: EffectSubject,
    },
    /// CR 613 layer 6: grant one or more keyword abilities to every creature matching `filter`
    /// until end of turn. Untargeted — the one-shot keyword-grant sibling of
    /// [`StaticAbilityDef::AnthemKeyword`]. Covers Overrun (Trample) and Make a Stand
    /// (Indestructible); attacking-creature keyword grants reuse the same snapshot filter.
    GrantKeywordsAll {
        #[serde(default)]
        filter: CreatureScopeFilter,
        keywords: Vec<Keyword>,
    },
    /// CR 613 layer 6: remove all abilities from the current snapshot of matching creatures
    /// until end of turn. Final Showdown and Vedalken Humiliator share this instruction.
    RemoveAbilitiesAll {
        #[serde(default)]
        filter: CreatureScopeFilter,
    },
    /// CR 613 layer 6: grant one or more keyword abilities until end of turn. `Chosen` is an
    /// ordinary permanent target (Boros Charm); `Source` auto-binds an activated or triggered
    /// ability to its own permanent without using the targeting path (Goblin Bird-Grabber).
    GrantKeywords {
        #[serde(default)]
        subject: EffectSubject,
        keywords: Vec<Keyword>,
    },
    /// Rattleback Apothecary and Golem Artisan choose one keyword on resolution, after targets
    /// have been chosen. Uses the same logged choice channel as GrantProtection.
    GrantKeywordChoice {
        #[serde(default)]
        subject: EffectSubject,
        choices: Vec<Keyword>,
    },
    /// CR 702.16 / layer 6: grant protection until end of turn. The explicit option list covers
    /// both color choices and mixed choices such as Apostle's Blessing.
    GrantProtection {
        #[serde(default)]
        subject: EffectSubject,
        protection: ProtectionGrant,
    },
    /// CR 611.2c / 613.1f: the subject gains a triggered ability until end of turn. The complete
    /// definition is embedded because the ability must keep functioning after the effect or its
    /// source leaves the battlefield. Abnormal Endurance and Fake Your Own Death share this.
    GrantTriggeredAbility {
        #[serde(default)]
        subject: EffectSubject,
        ability: Box<TriggeredAbilityDef>,
    },
    /// Return the event-bound card only from the immediate destination generation in `from`.
    /// Abnormal Endurance references the granted ability's source; Unholy Indenture and
    /// Earthbend reference the observed object. Later zone changes invalidate the return.
    ReturnTriggeredCard {
        reference: TriggeredCardReference,
        from: Vec<EventZone>,
        #[serde(default)]
        tapped: bool,
        #[serde(default)]
        controller: ReturnController,
        #[serde(default)]
        entry_counters: Vec<CounterPlacement>,
    },
    /// CR 603.7: create a one-shot delayed triggered ability that observes `subject`. The
    /// definition must use a delayed-only trigger condition.
    CreateDelayedTrigger {
        #[serde(default)]
        subject: EffectSubject,
        ability: Box<TriggeredAbilityDef>,
    },
    /// Sacrifice the exact generation-bound cohort captured by an engine-created delayed
    /// trigger. Mobilize uses this runtime context effect so one delayed ability handles all
    /// tokens made by one resolution without re-querying the battlefield by name.
    SacrificeObservedObjects,
    /// CR 702.185: exile the observed permanent incarnation and grant its owner Warp's
    /// later-turn permission. Runtime delayed effect shared by all Warp cards.
    ExileWarpedObject,
    /// CR 610.3 paired one-shot effects: exile the chosen permanent, then return that exact card
    /// under its owner's control immediately after the exact source generation leaves the
    /// battlefield. Banishing Light and Stormplain Detainment share the nonland form; Trapped in
    /// the Screen narrows it to artifact, creature, or enchantment.
    ExileUntilSourceLeaves {
        target: TargetFilter,
    },
    /// CR 205.1b / 613.1d: add card types or creature subtypes until end of turn without
    /// replacing the permanent's existing type line. Liquimetal Coating uses a chosen permanent;
    /// source- and event-bound ability effects reuse the same subject vocabulary.
    AddTypes {
        #[serde(default)]
        subject: EffectSubject,
        addition: TypeLineAddition,
    },
    /// CR 613 layer 6: grant keywords until end of turn to the permanents matching `filter`.
    /// The affected set is snapshotted as this effect resolves, rather than remaining dynamic.
    /// Covers Boros Charm and Heroic Intervention.
    GrantKeywordsAllPermanents {
        filter: TargetFilter,
        keywords: Vec<Keyword>,
    },
    /// Apply attack/block rules through the shared combat-legality path until cleanup. Source and
    /// chosen scopes bind one physical object; matching scopes remain dynamic because these
    /// restrictions modify the rules of the game rather than creature characteristics.
    ApplyCombatRestriction {
        scope: CombatRestrictionScope,
        restriction: CombatRestriction,
    },
    GainLife {
        amount: Amount,
    },
    /// CR 119.3: the players named by `who` lose life. **Untargeted** — neither "you" nor
    /// "each opponent" uses the word "target" (CR 115.1), so this is deliberately absent from
    /// `spell_effect_kind_needs_target` and [`SpellEffectKind::target_filters`]. Adding it to
    /// either would make every `LoseLife`-only card demand a target.
    ///
    /// Thoughtseize and Reanimate use the default `Controller`; Infectious Horror and Caged
    /// Zombie use `EachOpponent`. `EachPlayer` shares the same recipient vocabulary for cards
    /// that affect everyone without targeting.
    LoseLife {
        amount: LifeAmount,
        #[serde(default)]
        who: PlayerRecipient,
    },
    TargetPlayerGainsLife {
        amount: u32,
        target: TargetFilter,
    },
    TargetPlayerLosesLife {
        amount: u32,
        target: TargetFilter,
    },
    EachOpponentLosesLifeYouGainEqual {
        amount: u32,
    },
    /// Exile a permanent-valued subject. Haywire Mite and Oblivion Strike choose an ordinary
    /// target; Spiral into Solitude and Path to Redemption use the exact attached object without
    /// turning that instruction into a target.
    Exile {
        #[serde(default)]
        subject: EffectSubject,
    },
    /// Exile a permanent-valued subject and let the resulting card's owner cast it from exile
    /// for the stated alternative mana cost while that exact generation remains there. Airbend
    /// uses `{2}`; Release to the Wind demonstrates the same permission with a zero cost.
    ExileWithOwnerCastPermission {
        #[serde(default)]
        subject: EffectSubject,
        alternative_cost: ManaCost,
    },
    ExileTargetGainLifeEqualToPower,
    /// Exile the top card of the named player's library and let that player play the exact
    /// resulting object until the end of their next turn. Clockwork Percussionist and Impossible
    /// Inferno share this primitive; the engine owns physical identity, duration, and legality.
    ExileTopWithPlayPermission {
        player: PlayerRecipient,
        #[serde(default = "default_one")]
        count: u32,
        #[serde(default)]
        count_by_cast_cost: Option<CastCostConditionalAmount>,
    },
    /// Return a battlefield permanent to its owner's hand. A chosen subject uses the normal
    /// targeting contract (Unsummon, Boomerang); a source-bound subject is untargeted and keeps
    /// CR 400.7 generation identity (Wingspan Stride).
    ReturnToOwnersHand {
        subject: EffectSubject,
    },
    /// Move a permanent-valued subject to its owner's library (CR 400.3). Chosen subjects retain
    /// ordinary target legality; Watery Grasp uses the exact attached object untargeted.
    PutInOwnersLibrary {
        subject: EffectSubject,
        placement: LibraryPlacement,
    },
    /// Move a card from a graveyard to hand or battlefield (CR 400.1: graveyard is public).
    /// Raise Dead / Disentomb (creature → hand); future reanimation (creature → battlefield).
    /// The `filter` selects which graveyard and which card types are legal targets; the engine
    /// validates this at cast time and again at resolution (fizzle if no longer legal).
    MoveGraveyardCards {
        filter: GraveyardFilter,
        destination: GraveyardDestination,
    },
    /// Choose a card from the controller's graveyard when this instruction resolves rather than
    /// targeting it while casting. Say Its Name mills first, then optionally chooses the current
    /// creature-or-land cohort; Corpse Churn shares the same post-mill timing.
    ChooseGraveyardCard {
        filter: ZoneCardFilter,
        destination: GraveyardDestination,
        #[serde(default)]
        optional: bool,
        /// When present, candidates must be exact surviving entries from the immediately
        /// preceding card-moving instruction rather than the whole current graveyard.
        #[serde(default)]
        from_result: Option<CardResultFilter>,
    },
    MillTargetPlayer {
        count: u32,
        target: TargetFilter,
    },
    /// CR 701.17: mill the players named by `who` without targeting them. Gorging Vulture and
    /// Tribune of Rot use the default `Controller`; player-relative trigger effects can reuse the
    /// remaining [`PlayerRecipient`] variants without pretending that "you" is a target.
    Mill {
        count: Amount,
        #[serde(default)]
        who: PlayerRecipient,
    },
    /// CR 701.9: force `count` cards from the target player's hand to their graveyard.
    /// `chooser` distinguishes the affected-player default (Mind Rot), controller-selected
    /// revealed cards (Coercion, Thoughtseize), and seeded random choice (Hymn to Tourach).
    /// When `count` exceeds the hand size, the player discards all remaining cards.
    /// Cards covered: Mind Rot, Hymn to Tourach, Coercion, and Thoughtseize.
    DiscardCards {
        count: u32,
        target: TargetFilter,
        #[serde(default)]
        chooser: DiscardChooser,
        /// Optional card-type restriction on the chosen cards. The complete hand remains visible
        /// to the authorized chooser; legality is published separately.
        #[serde(default)]
        card_filter: Option<CardTypeFilter>,
        /// Whether the chooser may decline after seeing the hand.
        #[serde(default)]
        optional: bool,
        /// Whether the full hand window is chooser-private or publicly revealed while the choice
        /// remains pending. Selection legality is still carried separately by the engine.
        #[serde(default)]
        visibility: HandChoiceVisibility,
    },
    /// Choose cards from a target player's hand and exile them directly. This is deliberately
    /// distinct from [`SpellEffectKind::DiscardCards`]: it does not perform the CR 701.9 discard
    /// action and must not satisfy future discard triggers or replacement effects. Aggressive
    /// Negotiations and Elite Spellbinder share the controller-chosen nonland shape.
    ExileCardsFromHand {
        count: u32,
        target: TargetFilter,
        #[serde(default)]
        chooser: DiscardChooser,
        #[serde(default)]
        card_filter: Option<CardTypeFilter>,
        #[serde(default)]
        optional: bool,
        #[serde(default)]
        visibility: HandChoiceVisibility,
    },
    /// Destroy every battlefield permanent matching `kind` (CR 701.7). Untargeted, so it
    /// ignores hexproof/shroud and never fizzles. `kind` selects the affected set — `Creature`
    /// for Wrath of God / Day of Judgment, `AnyPermanent` for "destroy all permanents". Only
    /// object kinds are legal (validated at load); player kinds make no sense here.
    /// `prevent_regeneration: true` means regeneration shields are bypassed (Wrath of God:
    /// "they can't be regenerated", CR 701.19c).
    DestroyAll {
        #[serde(default = "TargetFilter::default_creature")]
        kind: TargetFilter,
        #[serde(default)]
        prevent_regeneration: bool,
    },
    /// CR 701.19: put a regeneration shield on target creature. The next time that creature would
    /// be destroyed this turn, instead tap it, remove it from combat, and clear all damage from it.
    /// Legal only as an activated ability effect — never a spell (validated at load). Covers
    /// Cudgel Troll (`{G}: Regenerate`) and Drudge Skeletons (`{B}: Regenerate`).
    Regenerate {
        #[serde(default)]
        subject: EffectSubject,
    },
    /// Deal `amount` damage to every battlefield permanent matching `kind` (CR 119). Untargeted.
    /// `Creature` covers Pyroclasm / Pestilence-style sweeps; a disjunction with planeswalkers
    /// covers Calamitous Cave-In. Only object kinds are legal (validated at load).
    DamageAll {
        amount: Amount,
        #[serde(default = "TargetFilter::default_creature")]
        kind: TargetFilter,
    },
    /// CR 111 / 707: create `count` tokens copying one targeted permanent under the
    /// resolving object's controller (Cackling Counterpart, Quasiduplicate).
    /// Copies use the source's copiable values, not later continuous modifications.
    CreateTokenCopies {
        count: Amount,
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
    },
    /// CR 701.36: choose a creature token you control during resolution, then copy it.
    /// Untargeted (Wake the Reflections, Rootborn Defenses).
    Populate,
    /// CR 701.47: if necessary create a black 0/0 Army creature token of `subtype`, choose an
    /// Army creature you control, put `count` +1/+1 counters on it, then add `subtype` if needed.
    /// Goblin-town Flunkies / Misty Mountains Raider and the established Zombie form share this
    /// non-targeting, resumable action.
    Amass {
        subtype: ArmySubtype,
        count: Amount,
    },
    /// CR 111: create `count` token permanents of the registry-defined [`token`](crate::token_def)
    /// under the chosen controller. Untargeted — the characteristics come from the
    /// [`TokenDefinition`](crate::token_def::TokenDefinition); only `count` and `who` vary per
    /// maker. Covers Raise the Alarm / Dragon Fodder (`Controller`, count 2), Curse of Opulence
    /// (`AttackingOpponentsOfDefendingPlayer`), and symmetrical makers (`EachPlayer`). Token
    /// *copies* of existing permanents (CR 707) are a separate effect.
    CreateTokens {
        /// Token id (slug of the token's name) in the registry's token namespace.
        token: String,
        count: Amount,
        #[serde(default)]
        who: PlayerRecipient,
        /// Whether the complete ordinary token cohort enters the battlefield tapped. This is
        /// initial entry status, not a later tap action (CR 110.5b).
        #[serde(default)]
        tapped: bool,
        #[serde(default)]
        sacrifice_timing: Option<DelayedTokenSacrificeTiming>,
    },
    /// CR 508.4 / 603.7: create a cohort under the resolving object's controller, tapped and
    /// attacking engine-chosen defending recipients. Each token's recipient is chosen separately;
    /// the complete cohort enters simultaneously and may create one delayed next-end-step
    /// sacrifice trigger. Mobilize cards such as Shock Brigade and Reigning Victor share this
    /// primitive. These creatures enter attacking but were never declared as attackers.
    CreateAttackingTokens {
        /// Token id in the registry's token namespace.
        token: String,
        count: Amount,
        #[serde(default)]
        sacrifice_timing: Option<DelayedTokenSacrificeTiming>,
    },
    /// CR 122/121.6: put `count` counters of `counter` on `subject` (default: a chosen creature
    /// target). The `counter` kind covers both +1/+1 counter spells
    /// (Battlegrowth, Common Bond) and -1/-1 counter spells (Instill Infection) without a new
    /// variant. Use `Source` for an ability that puts counters on its own source
    /// (modular/graft/outlast self-buffs). Counter *removal* spells are deferred — counter
    /// removal in MTG is almost always an ability cost (see the plan's `AbilityCost` phase).
    /// Mandatory CR 701.68 instruction. Unlike a Blight payment, it completes even when
    /// no creature can receive counters (Chaos Spewer; the same operation backs Dream Seizer).
    Blight {
        count: u32,
    },
    PutCounters {
        counter: CounterKind,
        count: Amount,
        #[serde(default)]
        subject: EffectSubject,
    },
    /// Heirloom Auntie and Reluctant Dounguard remove counters without paying a cost.
    RemoveCounters {
        counter: CounterKind,
        count: u32,
        #[serde(default)]
        subject: EffectSubject,
    },
    /// CR 122.8: recreate a departed object's counter bag, never move live counters.
    PutCounterSnapshot {
        from: CounterSnapshotSource,
        subject: EffectSubject,
    },
    /// CR 119 + 119.4: drain `amount` life from a target player and give that much life to the
    /// controller ("target player loses N life and you gain N life"). Covered by Blood Artist,
    /// Falkenrath Noble, and drain-life spells like Vampire's Kiss. The target must be a player.
    DrainTarget {
        amount: u32,
        target: TargetFilter,
    },
    /// CR 605 mana ability: add mana to the activating player's pool. Legal only as an
    /// activated ability's `effect` (never a spell `spell_effect`) — the engine classifies an
    /// ability with this effect as a mana ability (CR 605.1a), so it doesn't use the stack and
    /// resolves immediately. `options` lists the mutually exclusive bags of mana producible;
    /// one option = no choice (basic lands, Llanowar Elves, Sol Ring), several = the player
    /// picks at activation (dual/filter lands, any-color rocks). Untargeted.
    ProduceMana {
        options: Vec<ManaAmount>,
        /// CR 106.6: every pip produced by this ability carries the same spending restriction.
        /// Chandra's Embercat and Vodalian Arcanist are the first data consumers; the spell and
        /// ability branches also cover Castle Garenbrig without a card-specific primitive.
        #[serde(default)]
        restriction: Option<ManaSpendingRestriction>,
        /// A live replacement for `options` when its condition holds. Leafkin Druid is the first
        /// consumer. This is intentionally an "instead" branch rather than additive output.
        #[serde(default)]
        conditional: Option<ConditionalManaOutput>,
    },
    /// Add a fixed bag of mana while this spell or non-mana ability resolves. Unlike
    /// [`ProduceMana`](Self::ProduceMana), this effect uses the stack normally. Firebending and
    /// Radha, Heir to Keld are the first two mechanics supported by the shared retention shape.
    AddMana {
        amount: ManaAmount,
        retention: ManaRetention,
    },
    /// CR 701.18: pause resolution, let the casting player search their library for a card
    /// matching `filter` (None = any card; Some = only cards of that spell type), move it to
    /// `destination`, then shuffle if `shuffle` is true. Uses the tier-3 interrupt mechanism
    /// (`ResolutionChoiceRequired` / `SubmitResolutionChoice`) with `ChoiceKind::LibrarySearch`
    /// (private to the searching player). Named examples include Demonic Tutor (any → hand),
    /// Mystical Tutor (instant or sorcery → top of library), and Evolving Wilds (basic land →
    /// battlefield tapped).
    SearchLibrary {
        /// Number of cards to choose. Existing search effects default to one.
        #[serde(default = "default_one")]
        count: u32,
        /// Optional linked replacement for `count`, fixed by the cast-cost receipt. Grow from the
        /// Ashes searches for two basics when kicked and one otherwise.
        #[serde(default)]
        count_by_cast_cost: Option<CastCostConditionalAmount>,
        /// `None` = any card is valid; `Some(f)` = every authored predicate must match.
        #[serde(default)]
        filter: Option<ZoneCardFilter>,
        /// Independently filtered capacities. Empty selects homogeneous `count`/`filter` mode;
        /// nonempty selects heterogeneous slot mode and publishes only engine-authored edges.
        #[serde(default)]
        slots: Vec<SearchSelectionSlot>,
        /// Which of the controller's zones are searched. Existing tutors default to library;
        /// Say Its Name lets its controller choose any nonempty hand/graveyard/library subset.
        #[serde(default)]
        zones: SearchZoneSelection,
        /// Where the found card goes. Default: Hand.
        #[serde(default)]
        destination: SearchDestination,
        /// Live destination replacement evaluated only after the search choice is submitted.
        /// Embermouth Sentinel and Caravan Vigil share this conditional placement seam.
        #[serde(default)]
        conditional_destination: Option<ConditionalSearchDestination>,
        /// Shuffle the library after searching. Default: true (all canonical tutors shuffle).
        #[serde(default = "default_true")]
        shuffle: bool,
        /// Reveal the found card publicly (Mystical Tutor reveals to both players). Default: false.
        #[serde(default)]
        reveal: bool,
    },
    /// CR 301.5 / 702.6: the equip activated ability — attach this equipment to `target` creature
    /// you control. At resolution the engine moves `attached_to` on the equipment's `GameObject`
    /// to the new target (detaching from any previous creature automatically). The P/T bonus
    /// (if any) is a separate [`StaticAbilityDef::AttachedModifier`] that reads `attached_to`
    /// dynamically, so no continuous effect is updated on re-equip. Legal only as an activated
    /// ability's `effect`, never a spell effect; equip only as a sorcery (CR 702.6a).
    /// Covers Bonesplitter (equip {1}) and Vulshok Morningstar (equip {2}).
    Equip {
        #[serde(default = "TargetFilter::default_equip")]
        target: TargetFilter,
    },
    /// CR 701.17: force the targeted player to sacrifice a permanent matching `filter` (default:
    /// any creature). The target is specified by `target` (kind must be AnyPlayer or
    /// OpponentPlayer — validated at registry load). The targeted player chooses which qualifying
    /// permanent to sacrifice; if they have none the effect fizzles. Covers Diabolic Edict
    /// (opponent sacrifices a creature) and Plaguecrafter (target player sacrifices a creature).
    TargetPlayerSacrifices {
        /// Who must sacrifice — kind must be AnyPlayer or OpponentPlayer.
        target: TargetFilter,
        /// What kind of permanent may be sacrificed (default: Creature).
        #[serde(default = "TargetFilter::default_creature")]
        filter: TargetFilter,
    },
    /// CR 614.1a: place a prevention shield on the target (creature or player) that will absorb
    /// the next `amount` damage dealt to it this turn. Healing Salve mode 2 (`amount: 3,
    /// target: AnyTarget`); Circle of Protection variants use `amount: 1` and repeat.
    /// When damage would be dealt to a shielded object, subtract up to `amount` from the shield
    /// before recording the damage; the shield expires when fully consumed or at cleanup.
    PreventNextDamage {
        amount: u32,
        target: TargetFilter,
    },
    /// CR 615: prevent all combat damage that would be dealt to the targeted creature this turn.
    /// Unlike [`Self::PreventAllCombatDamageTurn`], this effect is the conjunction of one exact
    /// creature recipient and combat damage. Covers Fleeting Flight and the incoming-damage
    /// instruction of Azorius Ploy.
    PreventAllCombatDamageToTargetTurn {
        target: TargetFilter,
    },
    /// CR 614.1a: prevent all combat damage that would be dealt this turn (Fog, Holy Day,
    /// Safe Passage partial). Untargeted — sets a global flag checked when combat damage resolves.
    /// Cleared at the cleanup step alongside marked damage.
    PreventAllCombatDamageTurn,
    /// CR 615.12: damage cannot be prevented for the rest of the turn. Prevention effects still
    /// apply, including any additional effects, but prevent zero damage and do not consume finite
    /// shields. Cards: Stomp and Skullcrack (whose life-gain prohibition is a separate effect).
    DamageCantBePreventedThisTurn,
    /// CR 701.27 / 710: change the face/status of the permanent that sourced this ability.
    /// Ineligible objects produce the rules-defined no-op during resolution.
    ChangeSourceFace {
        action: FaceChangeAction,
    },
    /// Intrinsic CR 310.11b Siege defeat trigger. Engine-synthesized only; it moves the exact
    /// defeated Battle to exile and offers its controller the transformed free cast.
    SiegeDefeat,
    None,
}

fn default_true() -> bool {
    true
}

fn default_destroy_subject() -> EffectSubject {
    EffectSubject::Chosen(Box::new(TargetFilter::default_creature()))
}

/// Printed characteristics required of a card in a nonbattlefield zone. Leaf predicates compose
/// with AND semantics; `any_of` recursively joins two or more filters with OR. Tempest Hawk and
/// Living Phone use exact-name and printed-power leaves, while Say Its Name uses creature-or-land.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ZoneCardFilter {
    #[serde(default)]
    pub any_of: Option<Vec<Self>>,
    #[serde(default)]
    pub exact_name: Option<String>,
    #[serde(default)]
    pub card_type: Option<CardTypeFilter>,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub printed_power: Option<PowerComparison>,
}

impl ZoneCardFilter {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(branches) = &self.any_of {
            if branches.len() < 2 {
                return Err("zone card filter any_of requires at least two branches".into());
            }
            if self.exact_name.is_some()
                || self.card_type.is_some()
                || self.subtype.is_some()
                || self.printed_power.is_some()
            {
                return Err(
                    "zone card filter any_of cannot be combined with leaf predicates".into(),
                );
            }
            for branch in branches {
                branch.validate()?;
            }
            return Ok(());
        }
        if self.exact_name.is_none()
            && self.card_type.is_none()
            && self.subtype.is_none()
            && self.printed_power.is_none()
        {
            return Err("zone card filter requires at least one predicate".into());
        }
        if self
            .exact_name
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err("zone card filter exact name cannot be empty".into());
        }
        if self
            .subtype
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err("zone card filter subtype cannot be empty".into());
        }
        Ok(())
    }
}

/// Where a searched-out card goes (for [`SpellEffectKind::SearchLibrary`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SearchDestination {
    /// The card goes to the searching player's hand (Demonic Tutor, Cultivate).
    #[default]
    Hand,
    /// The card is placed on top of the searching player's library (Mystical Tutor).
    TopOfLibrary,
    /// The card enters the battlefield under the searching player's control. Entry replacement
    /// effects are applied before the search resumes and shuffles (Evolving Wilds, Rampant Growth).
    Battlefield {
        /// Whether the search effect instructs the card to enter tapped.
        #[serde(default)]
        tapped: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardSearchZone {
    Hand,
    Graveyard,
    Library,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchZoneSelection {
    Fixed(Vec<CardSearchZone>),
    PlayerChoice(Vec<CardSearchZone>),
}

impl Default for SearchZoneSelection {
    fn default() -> Self {
        Self::Fixed(vec![CardSearchZone::Library])
    }
}

impl SearchZoneSelection {
    fn validate(&self) -> Result<(), String> {
        let zones = match self {
            Self::Fixed(zones) | Self::PlayerChoice(zones) => zones,
        };
        if zones.is_empty() {
            return Err("zone search requires at least one available zone".into());
        }
        let mut unique = zones.clone();
        unique.sort_by_key(|zone| *zone as u8);
        unique.dedup();
        if unique.len() != zones.len() {
            return Err("zone search zones must be distinct".into());
        }
        if matches!(self, Self::PlayerChoice(_)) && zones.len() < 2 {
            return Err("player-selected zone search requires at least two zones".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionalSearchDestination {
    pub condition: GameCondition,
    pub destination: SearchDestination,
}

/// One bag of mana a mana ability can produce (CR 106): a count per mana type. A mana ability's
/// [`SpellEffectKind::ProduceMana`] carries a `Vec<ManaAmount>` of *options*; with one option the
/// ability produces it unconditionally (basic Forest `(g: 1)`, Sol Ring `(c: 2)`, Llanowar Elves
/// `(g: 1)`), with several the activating player picks one (a dual land's two colors; "any color"
/// enumerated as five single-color options). Serde defaults every field to 0 so RON lists only the
/// nonzero types (`(g: 1)`, `(w: 1, u: 1)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ManaAmount {
    #[serde(default)]
    pub w: u32,
    #[serde(default)]
    pub u: u32,
    #[serde(default)]
    pub b: u32,
    #[serde(default)]
    pub r: u32,
    #[serde(default)]
    pub g: u32,
    #[serde(default)]
    pub c: u32,
}

/// The boundary at which resolving mana stops being retained. `EndOfStep` follows the ordinary
/// CR 106.4 rule; `EndOfCombat` supports firebending's explicit exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManaRetention {
    EndOfStep,
    EndOfCombat,
}

/// One OR-branch in a spending restriction. A branch may constrain card type, subtype, or both;
/// every specified predicate must match. Chandra's Embercat uses two branches (Elemental OR
/// Chandra planeswalker), while Vodalian Arcanist uses one InstantOrSorcery branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManaSpendFilter {
    #[serde(default)]
    pub card_type: Option<CardTypeFilter>,
    #[serde(default)]
    pub subtype: Option<String>,
}

impl ManaSpendFilter {
    pub fn validate(&self) -> Result<(), String> {
        if self.card_type.is_none() && self.subtype.as_deref().is_none_or(str::is_empty) {
            return Err("mana spending filter requires a card type or subtype".into());
        }
        if self.subtype.as_deref().is_some_and(str::is_empty) {
            return Err("mana spending filter subtype cannot be empty".into());
        }
        Ok(())
    }

    fn fallback_card_description(&self) -> String {
        let type_name = self.card_type.map(|card_type| match card_type {
            CardTypeFilter::BasicLand => "basic land",
            CardTypeFilter::Land => "land",
            CardTypeFilter::Enchantment => "enchantment",
            CardTypeFilter::Instant => "instant",
            CardTypeFilter::Sorcery => "sorcery",
            CardTypeFilter::InstantOrSorcery => "instant or sorcery",
            CardTypeFilter::Creature => "creature",
            CardTypeFilter::Artifact => "artifact",
            CardTypeFilter::Planeswalker => "planeswalker",
            CardTypeFilter::Battle => "battle",
            CardTypeFilter::Nonland => "nonland",
            CardTypeFilter::NonlandPermanent => "nonland permanent",
            CardTypeFilter::Noncreature => "noncreature",
        });
        let description = match (self.subtype.as_deref(), type_name) {
            (Some(subtype), Some(card_type)) => format!("{subtype} {card_type}"),
            (Some(subtype), None) => subtype.to_owned(),
            (None, Some(card_type)) => card_type.to_owned(),
            (None, None) => "matching".into(),
        };
        let article =
            if description.chars().next().is_some_and(|first| {
                matches!(first.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u')
            }) {
                "an"
            } else {
                "a"
            };
        format!("{article} {description}")
    }

    fn fallback_spell_description(&self) -> String {
        format!("{} spell", self.fallback_card_description())
    }
}

/// Payment purposes for the existing Room-unlock and manifest face-up special actions.
/// Kept separate from special-action prohibitions: these permissions constrain mana, not actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecialActionManaPurpose {
    UnlockRoomDoor,
    TurnFaceUp,
}

/// CR 106.6 restriction carried by an individual mana contribution. A purpose is disallowed
/// unless its filter list or broader purpose flag permits it. Filters within one list are ORed so
/// one contribution can cover wording such as "an Elemental spell or a Chandra planeswalker
/// spell."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManaSpendingRestriction {
    pub restriction_id: ChoiceId,
    pub presentation: AbilityPresentation,
    #[serde(default)]
    pub cast_spell: Vec<ManaSpendFilter>,
    #[serde(default)]
    pub activate_ability: Vec<ManaSpendFilter>,
    /// This mana may pay for any activated ability, regardless of the source's characteristics.
    /// Purple Dragon Punks is the first consumer. Keep this separate from `activate_ability` so
    /// an empty characteristic filter never ambiguously means either "any" or "invalid".
    #[serde(default)]
    pub activate_any_ability: bool,
    /// This mana may pay any cost whose purpose is not casting a spell. Hydraulic Helper is the
    /// first consumer; this includes activated abilities, special actions, Ward, and authored
    /// resolution payments while leaving spell additional costs classified with their cast.
    #[serde(default)]
    pub all_nonspell_costs: bool,
    #[serde(default)]
    pub special_actions: Vec<SpecialActionManaPurpose>,
}

impl ManaSpendingRestriction {
    pub fn validate(&self) -> Result<(), String> {
        self.restriction_id.validate()?;
        self.presentation.validate()?;
        if self.cast_spell.is_empty()
            && self.activate_ability.is_empty()
            && !self.activate_any_ability
            && !self.all_nonspell_costs
            && self.special_actions.is_empty()
        {
            return Err("mana spending restriction must allow a spending purpose".into());
        }
        if self.activate_any_ability && !self.activate_ability.is_empty() {
            return Err(
                "activate_any_ability cannot be combined with filtered ability permissions".into(),
            );
        }
        if self.all_nonspell_costs
            && (self.activate_any_ability
                || !self.activate_ability.is_empty()
                || !self.special_actions.is_empty())
        {
            return Err(
                "all_nonspell_costs cannot be combined with narrower nonspell permissions".into(),
            );
        }
        self.cast_spell
            .iter()
            .chain(&self.activate_ability)
            .try_for_each(ManaSpendFilter::validate)
    }

    pub fn fallback_label(&self) -> String {
        let mut purposes = self
            .cast_spell
            .iter()
            .map(|filter| format!("cast {}", filter.fallback_spell_description()))
            .chain(
                self.activate_any_ability
                    .then_some("activate an ability".into()),
            )
            .chain(self.activate_ability.iter().map(|filter| {
                format!(
                    "activate an ability of {}",
                    filter.fallback_card_description()
                )
            }))
            .collect::<Vec<_>>();
        purposes.extend(self.special_actions.iter().map(|purpose| match purpose {
            SpecialActionManaPurpose::UnlockRoomDoor => "unlock a door".into(),
            SpecialActionManaPurpose::TurnFaceUp => "turn a permanent face up".into(),
        }));
        if self.all_nonspell_costs {
            purposes.push("pay a nonspell cost".into());
        }
        let joined = match purposes.as_slice() {
            [] => choice_fallback("Restricted mana", &self.restriction_id),
            [only] => only.clone(),
            [first, second] => format!("{first} or {second}"),
            _ => {
                let (last, rest) = purposes.split_last().expect("nonempty purpose list");
                format!("{}, or {last}", rest.join(", "))
            }
        };
        format!("Spend only to {joined}")
    }
}

/// Live conditional replacement for a mana ability's ordinary options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionalManaOutput {
    pub condition: GameCondition,
    pub options: Vec<ManaAmount>,
}

/// Which players' creatures a mass one-shot effect affects, relative to the effect controller.
/// Kept separate from target selection because these effects do not target. Covers Cryptic
/// Command / Tempest Caller (`Opponents`) and controller-only mass tap/untap effects (`Controller`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelativePlayerSet {
    Controller,
    Opponents,
    All,
}

/// Which player an **untargeted** effect affects.
///
/// Sibling of [`RelativePlayerSet`], and kept out of `TargetFilter` for the same reason: naming a
/// player is not targeting it (CR 115.1), and borrowing
/// targeting vocabulary for effects that do not target is what forced source-bound effects to
/// masquerade as targets before [`EffectSubject`] separated the concepts. Sulfuric Vortex never
/// says "target".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlayerRecipient {
    /// "you" — the spell or ability's controller. Serendib Efreet, Juzám Djinn.
    #[default]
    Controller,
    /// "that player" — the player the resolving item is *about*: a trigger's
    /// `StackItem::trigger_context.affected_player` (whose upkeep/draw step it is), falling back
    /// to the controller when the item names nobody. Sulfuric Vortex, Underworld Dreams, Ebony
    /// Owl Netsuke.
    AffectedPlayer,
    /// The current controller of the permanent named by the trigger event, using its controller
    /// at the event as last known information if that object has since left the battlefield.
    TriggerObjectController,
    /// The current controller of the resolving item's source object, falling back to
    /// generation-keyed last known information if that exact object has left the battlefield.
    SourceController,
    /// The current controller of the sole legal permanent in an authored target group. Chandra's
    /// Outrage and Searing Blaze-style effects evaluate this relationship at resolution rather
    /// than snapshotting the target's controller when the spell was cast.
    ControllerOfTargetGroup { group_index: u32 },
    /// The event-time defending player of the attack that caused this trigger. Scorch Spitter and
    /// similar attack triggers keep that player even if the source leaves before resolution.
    DefendingPlayer,
    /// The event-time attacking player when that player is an opponent of the defender and still
    /// controls at least one current attacker. Curse of Opulence and Curse of Disturbance use this
    /// for their one-per-attack-event reward.
    AttackingOpponentsOfDefendingPlayer,
    /// "each opponent" — every other player still in the game. Pestilence-style drains.
    EachOpponent,
    /// "each player" — everyone still in the game, controller included. Earthquake's player half.
    EachPlayer,
}

/// The fixed target contract of Earthbend (Badgermole and Dai Li Indoctrination).
pub fn earthbend_target_filter() -> &'static TargetFilter {
    static FILTER: std::sync::LazyLock<TargetFilter> = std::sync::LazyLock::new(|| TargetFilter {
        kind: TargetKind::AnyPermanent,
        controller: TargetController::You,
        permanent_types: vec![PermanentTypeFilter::Land],
        ..TargetFilter::default()
    });
    &FILTER
}

impl SpellEffectKind {
    pub(crate) fn uses_attached_object_subject(&self) -> bool {
        matches!(
            self,
            SpellEffectKind::PumpTarget {
                subject: EffectSubject::AttachedObject,
                ..
            } | SpellEffectKind::PutCounters {
                subject: EffectSubject::AttachedObject,
                ..
            } | SpellEffectKind::RemoveCounters {
                subject: EffectSubject::AttachedObject,
                ..
            } | SpellEffectKind::PutCounterSnapshot {
                subject: EffectSubject::AttachedObject,
                ..
            } | SpellEffectKind::GrantKeywords {
                subject: EffectSubject::AttachedObject,
                ..
            } | SpellEffectKind::GrantKeywordChoice {
                subject: EffectSubject::AttachedObject,
                ..
            } | SpellEffectKind::GrantTriggeredAbility {
                subject: EffectSubject::AttachedObject,
                ..
            } | SpellEffectKind::CreateDelayedTrigger {
                subject: EffectSubject::AttachedObject,
                ..
            } | SpellEffectKind::AddTypes {
                subject: EffectSubject::AttachedObject,
                ..
            } | SpellEffectKind::Tap {
                subject: EffectSubject::AttachedObject,
            } | SpellEffectKind::Untap {
                subject: EffectSubject::AttachedObject,
            } | SpellEffectKind::Regenerate {
                subject: EffectSubject::AttachedObject,
            } | SpellEffectKind::Fight {
                first: EffectSubject::AttachedObject,
                ..
            } | SpellEffectKind::Fight {
                second: EffectSubject::AttachedObject,
                ..
            } | SpellEffectKind::ReturnToOwnersHand {
                subject: EffectSubject::AttachedObject,
            } | SpellEffectKind::Exile {
                subject: EffectSubject::AttachedObject,
            } | SpellEffectKind::ExileWithOwnerCastPermission {
                subject: EffectSubject::AttachedObject,
                ..
            } | SpellEffectKind::PutInOwnersLibrary {
                subject: EffectSubject::AttachedObject,
                ..
            } | SpellEffectKind::Destroy {
                subject: EffectSubject::AttachedObject,
            }
        )
    }

    pub(crate) fn uses_trigger_object_reference(&self) -> bool {
        matches!(
            self,
            SpellEffectKind::PutCounterSnapshot {
                from: CounterSnapshotSource::TriggerObject,
                ..
            } | SpellEffectKind::PumpTarget {
                subject: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::PutCounters {
                subject: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::RemoveCounters {
                subject: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::PutCounterSnapshot {
                subject: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::GrantKeywords {
                subject: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::GrantKeywordChoice {
                subject: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::GrantTriggeredAbility {
                subject: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::CreateDelayedTrigger {
                subject: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::AddTypes {
                subject: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::Tap {
                subject: EffectSubject::TriggerObject,
            } | SpellEffectKind::Untap {
                subject: EffectSubject::TriggerObject,
            } | SpellEffectKind::Regenerate {
                subject: EffectSubject::TriggerObject,
            } | SpellEffectKind::Fight {
                first: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::Fight {
                second: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::ReturnToOwnersHand {
                subject: EffectSubject::TriggerObject,
            } | SpellEffectKind::Exile {
                subject: EffectSubject::TriggerObject,
            } | SpellEffectKind::ExileWithOwnerCastPermission {
                subject: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::PutInOwnersLibrary {
                subject: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::Destroy {
                subject: EffectSubject::TriggerObject,
            } | SpellEffectKind::LoseLife {
                who: PlayerRecipient::TriggerObjectController,
                ..
            } | SpellEffectKind::DamagePlayer {
                who: PlayerRecipient::TriggerObjectController,
                ..
            } | SpellEffectKind::CreateTokens {
                who: PlayerRecipient::TriggerObjectController,
                ..
            } | SpellEffectKind::Mill {
                who: PlayerRecipient::TriggerObjectController,
                ..
            } | SpellEffectKind::ReturnTriggeredCard {
                reference: TriggeredCardReference::TriggerObject,
                ..
            }
        )
    }

    pub(crate) fn uses_defending_player_reference(&self) -> bool {
        self.target_filters().iter().any(|filter| {
            filter.any_terminal_filter_matches(|leaf| {
                leaf.controller == TargetController::DefendingPlayer
            })
        }) || matches!(
            self,
            SpellEffectKind::LoseLife {
                who: PlayerRecipient::DefendingPlayer
                    | PlayerRecipient::AttackingOpponentsOfDefendingPlayer,
                ..
            } | SpellEffectKind::DamagePlayer {
                who: PlayerRecipient::DefendingPlayer
                    | PlayerRecipient::AttackingOpponentsOfDefendingPlayer,
                ..
            } | SpellEffectKind::Mill {
                who: PlayerRecipient::DefendingPlayer
                    | PlayerRecipient::AttackingOpponentsOfDefendingPlayer,
                ..
            } | SpellEffectKind::CreateTokens {
                who: PlayerRecipient::DefendingPlayer
                    | PlayerRecipient::AttackingOpponentsOfDefendingPlayer,
                ..
            }
        )
    }

    pub(crate) fn uses_targeting_stack_reference(&self) -> bool {
        matches!(
            self,
            SpellEffectKind::CounterTriggeringStackObjectUnlessPays { .. }
        )
    }

    pub fn needs_target(&self) -> bool {
        !self.target_roles().is_empty()
    }

    /// Exhaustive semantic target contract for this effect. Every enum variant is named here so a
    /// new primitive cannot compile until it explicitly declares its target roles (or lack of
    /// them). Group cardinality and role binding are compiled by [`super::TargetSchema`].
    pub fn target_roles(&self) -> Vec<TargetRole<'_>> {
        match self {
            SpellEffectKind::Conditional { effect, .. }
            | SpellEffectKind::ConditionalCastCost { effect, .. } => effect.target_roles(),
            SpellEffectKind::Earthbend { .. } => {
                vec![TargetRole::Filtered(earthbend_target_filter())]
            }
            SpellEffectKind::CreatureDealsDamageEqualToPower { source, target } => {
                vec![TargetRole::Filtered(source), TargetRole::Filtered(target)]
            }
            SpellEffectKind::SetBasePowerToughness { target, .. } => {
                vec![TargetRole::Filtered(target)]
            }
            SpellEffectKind::Fight { first, second } => [first, second]
                .into_iter()
                .filter_map(|subject| match subject {
                    EffectSubject::Chosen(filter) => Some(TargetRole::Filtered(filter)),
                    EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject
                    | EffectSubject::PreviousEffectObject => None,
                })
                .collect(),
            SpellEffectKind::AttachEquipment {
                equipment,
                creature,
            } => [equipment, creature]
                .into_iter()
                .filter_map(|subject| match subject {
                    EffectSubject::Chosen(filter) => Some(TargetRole::Filtered(filter)),
                    EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject
                    | EffectSubject::PreviousEffectObject => None,
                })
                .collect(),
            SpellEffectKind::Destroy { subject }
            | SpellEffectKind::Sacrifice { subject }
            | SpellEffectKind::PumpTarget { subject, .. }
            | SpellEffectKind::Tap { subject }
            | SpellEffectKind::Untap { subject }
            | SpellEffectKind::GrantKeywords { subject, .. }
            | SpellEffectKind::GrantKeywordChoice { subject, .. }
            | SpellEffectKind::GrantProtection { subject, .. }
            | SpellEffectKind::GrantTriggeredAbility { subject, .. }
            | SpellEffectKind::CreateDelayedTrigger { subject, .. }
            | SpellEffectKind::AddTypes { subject, .. }
            | SpellEffectKind::ReturnToOwnersHand { subject }
            | SpellEffectKind::Exile { subject }
            | SpellEffectKind::ExileWithOwnerCastPermission { subject, .. }
            | SpellEffectKind::PutInOwnersLibrary { subject, .. }
            | SpellEffectKind::Regenerate { subject }
            | SpellEffectKind::PutCounters { subject, .. }
            | SpellEffectKind::RemoveCounters { subject, .. }
            | SpellEffectKind::PutCounterSnapshot { subject, .. } => match subject {
                EffectSubject::Chosen(target) => vec![TargetRole::Filtered(target)],
                EffectSubject::Source
                | EffectSubject::AttachedObject
                | EffectSubject::TriggerObject
                | EffectSubject::PreviousEffectObject => Vec::new(),
            },
            SpellEffectKind::ApplyCombatRestriction { scope, .. } => match scope {
                CombatRestrictionScope::Chosen(target) => vec![TargetRole::Filtered(target)],
                CombatRestrictionScope::Source | CombatRestrictionScope::Matching(_) => Vec::new(),
            },
            SpellEffectKind::DamageTarget { target, .. }
            | SpellEffectKind::CreateTokenCopies { target, .. }
            | SpellEffectKind::ExileIfWouldDieThisTurn { target }
            | SpellEffectKind::DamageTargets { target, .. }
            | SpellEffectKind::DestroyAttached { target, .. }
            | SpellEffectKind::SkipNextUntap { target }
            | SpellEffectKind::GainControlUntilEndOfTurn { target }
            | SpellEffectKind::TargetPlayerGainsLife { target, .. }
            | SpellEffectKind::TargetPlayerLosesLife { target, .. }
            | SpellEffectKind::TargetPlayerDraws { target, .. }
            | SpellEffectKind::DrainTarget { target, .. }
            | SpellEffectKind::MillTargetPlayer { target, .. }
            | SpellEffectKind::DiscardCards { target, .. }
            | SpellEffectKind::ExileCardsFromHand { target, .. }
            | SpellEffectKind::ExileUntilSourceLeaves { target }
            | SpellEffectKind::AuraAttach { target }
            | SpellEffectKind::AttachSource { target }
            | SpellEffectKind::Equip { target }
            | SpellEffectKind::TargetPlayerSacrifices { target, .. }
            | SpellEffectKind::PreventNextDamage { target, .. }
            | SpellEffectKind::PreventAllCombatDamageToTargetTurn { target } => {
                vec![TargetRole::Filtered(target)]
            }
            SpellEffectKind::ExileTargetGainLifeEqualToPower => {
                vec![TargetRole::CreaturePermanent]
            }
            SpellEffectKind::CounterTargetSpell { spell_filter, .. }
            | SpellEffectKind::CopyTargetSpell { spell_filter, .. } => {
                vec![TargetRole::StackSpell(spell_filter)]
            }
            SpellEffectKind::MoveGraveyardCards { filter, .. } => {
                vec![TargetRole::GraveyardCard(filter)]
            }
            SpellEffectKind::DamagePlayer { .. }
            | SpellEffectKind::DamageAttackedPlayerOrPlaneswalker { .. }
            | SpellEffectKind::Draw { .. }
            | SpellEffectKind::Discard { .. }
            | SpellEffectKind::DrawDiscard { .. }
            | SpellEffectKind::Blight { .. }
            | SpellEffectKind::CounterTriggeringStackObjectUnlessPays { .. }
            | SpellEffectKind::ChooseResolutionBranch { .. }
            | SpellEffectKind::ChoosePermanents { .. }
            | SpellEffectKind::CreateReflexiveTrigger { .. }
            | SpellEffectKind::Scry { .. }
            | SpellEffectKind::LibraryPartition { .. }
            | SpellEffectKind::ManifestDread
            | SpellEffectKind::LookChooseToHand { .. }
            | SpellEffectKind::TapAllCreatures { .. }
            | SpellEffectKind::UntapAll { .. }
            | SpellEffectKind::PumpAll { .. }
            | SpellEffectKind::GrantKeywordsAll { .. }
            | SpellEffectKind::RemoveAbilitiesAll { .. }
            | SpellEffectKind::ReturnTriggeredCard { .. }
            | SpellEffectKind::SacrificeObservedObjects
            | SpellEffectKind::ExileWarpedObject
            | SpellEffectKind::ChooseGraveyardCard { .. }
            | SpellEffectKind::GrantKeywordsAllPermanents { .. }
            | SpellEffectKind::GainLife { .. }
            | SpellEffectKind::LoseLife { .. }
            | SpellEffectKind::EachOpponentLosesLifeYouGainEqual { .. }
            | SpellEffectKind::ExileTopWithPlayPermission { .. }
            | SpellEffectKind::Mill { .. }
            | SpellEffectKind::DestroyAll { .. }
            | SpellEffectKind::DamageAll { .. }
            | SpellEffectKind::CreateTokens { .. }
            | SpellEffectKind::Amass { .. }
            | SpellEffectKind::Populate
            | SpellEffectKind::CreateAttackingTokens { .. }
            | SpellEffectKind::ProduceMana { .. }
            | SpellEffectKind::AddMana { .. }
            | SpellEffectKind::SearchLibrary { .. }
            | SpellEffectKind::PreventAllCombatDamageTurn
            | SpellEffectKind::DamageCantBePreventedThisTurn
            | SpellEffectKind::ChangeSourceFace { .. }
            | SpellEffectKind::SiegeDefeat
            | SpellEffectKind::None => Vec::new(),
        }
    }

    pub fn target_filters(&self) -> Vec<&TargetFilter> {
        self.target_roles()
            .into_iter()
            .filter_map(|role| match role {
                TargetRole::Filtered(filter) => Some(filter),
                TargetRole::CreaturePermanent
                | TargetRole::StackSpell(_)
                | TargetRole::GraveyardCard(_) => None,
            })
            .collect()
    }

    /// True if this effect selects an *object* target (not a player). Used by the effect-list
    /// validation below to decide whether a `LifeAmount::TargetManaValue` has something to read.
    fn targets_an_object(&self) -> bool {
        self.target_roles()
            .into_iter()
            .any(TargetRole::targets_an_object)
    }

    /// Face-local references share the existing amount and branch consumers. Nested branches
    /// retain the spell's context; created/granted abilities validate independently as abilities.
    pub(crate) fn validate_cast_snapshot_references(&self, count: usize) -> Result<(), String> {
        match self {
            Self::DamageTarget { amount, .. }
            | Self::DamageAll { amount, .. }
            | Self::Scry { count: amount }
            | Self::Earthbend { count: amount }
            | Self::CounterTargetSpell {
                unless_controller_pays: Some(amount),
                ..
            }
            | Self::DamageTargets { amount, .. }
            | Self::DamagePlayer { amount, .. }
            | Self::DamageAttackedPlayerOrPlaneswalker { amount }
            | Self::Draw { count: amount, .. }
            | Self::GainLife { amount }
            | Self::Mill { count: amount, .. }
            | Self::PutCounters { count: amount, .. }
            | Self::Amass { count: amount, .. }
            | Self::CreateTokens { count: amount, .. }
            | Self::CreateTokenCopies { count: amount, .. }
            | Self::CreateAttackingTokens { count: amount, .. } => {
                amount.validate_cast_snapshot_references(count)?;
            }
            Self::PumpTarget {
                scale: Some(scale), ..
            } => {
                scale.validate_cast_snapshot_references(count)?;
            }
            Self::ChooseResolutionBranch { branches, .. } => {
                for branch in branches {
                    if let ResolutionBranchRequirement::GameCondition(condition) =
                        &branch.requirement
                    {
                        condition.validate_cast_snapshot_reference(count)?;
                    }
                    for effect in &branch.effects {
                        effect.validate_cast_snapshot_references(count)?;
                    }
                }
            }
            Self::Conditional { condition, effect } => {
                condition.validate_cast_snapshot_reference(count)?;
                effect.validate_cast_snapshot_references(count)?;
            }
            Self::ConditionalCastCost { effect, .. } => {
                effect.validate_cast_snapshot_references(count)?;
            }
            Self::SearchLibrary {
                conditional_destination: Some(conditional),
                ..
            } => {
                conditional
                    .condition
                    .validate_cast_snapshot_reference(count)?;
            }
            Self::ProduceMana {
                conditional: Some(conditional),
                ..
            } => {
                conditional.condition.validate_live()?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Startup validation for a whole effect list (one face's `spell_effect`, or one mode's
    /// effects). Rules that depend on *sibling* effects live here; per-effect rules live in
    /// [`SpellEffectKind::validate`], which the caller runs too.
    pub fn validate_list(effects: &[SpellEffectKind]) -> Result<(), String> {
        fn produces_card_result(effect: &SpellEffectKind, action: CardResultAction) -> bool {
            match action {
                CardResultAction::Discard => matches!(
                    effect,
                    SpellEffectKind::Discard { .. }
                        | SpellEffectKind::DiscardCards { .. }
                        | SpellEffectKind::DrawDiscard { .. }
                ),
                CardResultAction::Destroy => matches!(effect, SpellEffectKind::Destroy { .. }),
                CardResultAction::Exile => matches!(
                    effect,
                    SpellEffectKind::ExileCardsFromHand { .. }
                        | SpellEffectKind::ExileTopWithPlayPermission { .. }
                        | SpellEffectKind::ExileWithOwnerCastPermission { .. }
                        | SpellEffectKind::MoveGraveyardCards {
                            destination: GraveyardDestination::Exile,
                            ..
                        }
                ),
                CardResultAction::Sacrifice => {
                    matches!(effect, SpellEffectKind::TargetPlayerSacrifices { .. })
                }
                CardResultAction::Mill => matches!(
                    effect,
                    SpellEffectKind::Mill { .. } | SpellEffectKind::MillTargetPlayer { .. }
                ),
                CardResultAction::Tap => false,
            }
        }

        if effects
            .iter()
            .filter(|effect| matches!(effect, SpellEffectKind::ChooseResolutionBranch { .. }))
            .count()
            > 1
        {
            return Err("an effect list may contain at most one resolution branch choice".into());
        }
        // `TargetManaValue` reads the mana value of the spell's target, so the list must contain
        // an effect that declares an object target (Reanimate: `MoveGraveyardCards`). Without
        // one there is nothing to read and the amount would silently resolve to 0.
        if effects.iter().any(|e| {
            matches!(
                e,
                SpellEffectKind::LoseLife {
                    amount: LifeAmount::TargetManaValue,
                    ..
                }
            )
        }) && !effects.iter().any(|e| e.targets_an_object())
        {
            return Err(
                "LoseLife(amount: TargetManaValue) requires an object-targeting effect in the \
                 same effect list to read the mana value from"
                    .into(),
            );
        }

        for (index, effect) in effects.iter().enumerate() {
            let amount = match effect {
                SpellEffectKind::DamageTarget { amount, .. }
                | SpellEffectKind::DamageAll { amount, .. }
                | SpellEffectKind::Scry { count: amount }
                | SpellEffectKind::Earthbend { count: amount }
                | SpellEffectKind::CounterTargetSpell {
                    unless_controller_pays: Some(amount),
                    ..
                }
                | SpellEffectKind::DamagePlayer { amount, .. }
                | SpellEffectKind::Draw { count: amount, .. }
                | SpellEffectKind::GainLife { amount }
                | SpellEffectKind::Mill { count: amount, .. }
                | SpellEffectKind::PutCounters { count: amount, .. }
                | SpellEffectKind::Amass { count: amount, .. }
                | SpellEffectKind::CreateTokens { count: amount, .. }
                | SpellEffectKind::CreateTokenCopies { count: amount, .. }
                | SpellEffectKind::CreateAttackingTokens { count: amount, .. } => Some(amount),
                SpellEffectKind::PumpTarget {
                    scale: Some(scale), ..
                } => scale.amount(),
                _ => None,
            };
            let previous = index
                .checked_sub(1)
                .and_then(|previous| effects.get(previous));
            if matches!(
                effect,
                SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::PreviousEffectObject,
                    ..
                }
            ) && !matches!(
                previous,
                Some(SpellEffectKind::ChoosePermanents { min: 1, max: 1, .. })
            ) {
                return Err(
                    "PreviousEffectObject requires an immediately preceding exactly-one ChoosePermanents"
                        .into(),
                );
            }
            let previous_is_at_most_one_permanent_choice = match previous {
                Some(SpellEffectKind::ChoosePermanents { max: 1, .. }) => true,
                Some(SpellEffectKind::Conditional { effect, .. }) => matches!(
                    effect.as_ref(),
                    SpellEffectKind::ChoosePermanents { max: 1, .. }
                ),
                _ => false,
            };
            if matches!(
                effect,
                SpellEffectKind::AttachEquipment {
                    equipment: EffectSubject::PreviousEffectObject,
                    ..
                }
            ) && !previous_is_at_most_one_permanent_choice
            {
                return Err(
                    "AttachEquipment PreviousEffectObject requires an immediately preceding at-most-one ChoosePermanents"
                        .into(),
                );
            }
            if let Some(filter) = amount.and_then(Amount::card_result_filter) {
                if filter.source == CardResultSource::PreviousEffect
                    && !previous.is_some_and(|effect| produces_card_result(effect, filter.action))
                {
                    return Err(
                        "PreviousEffect card result requires an immediately preceding compatible card-moving effect"
                            .into(),
                    );
                }
            }
            if let SpellEffectKind::ChooseResolutionBranch { branches, .. } = effect {
                for branch in branches {
                    if let ResolutionBranchRequirement::CardResultCount { filter, .. } =
                        &branch.requirement
                    {
                        if filter.source == CardResultSource::PreviousEffect
                            && !previous
                                .is_some_and(|effect| produces_card_result(effect, filter.action))
                        {
                            return Err(
                                "PreviousEffect card result requires an immediately preceding compatible card-moving effect"
                                    .into(),
                            );
                        }
                    }
                }
            }
            if let SpellEffectKind::ChooseGraveyardCard {
                from_result: Some(filter),
                ..
            } = effect
            {
                if filter.source != CardResultSource::PreviousEffect
                    || !previous.is_some_and(|effect| produces_card_result(effect, filter.action))
                {
                    return Err(
                        "ChooseGraveyardCard result source requires an immediately preceding compatible card-moving effect"
                            .into(),
                    );
                }
            }
        }
        Ok(())
    }

    /// Startup validation: reject effect/filter combinations the engine cannot honor.
    /// Returns `Err` with a human-readable reason; called from the card registry loader.
    /// `context` distinguishes spells from abilities so source-bound subjects are
    /// rejected where they make no sense.
    pub fn validate(&self, context: EffectContext) -> Result<(), String> {
        if context == EffectContext::Spell && self.requires_triggering_spell_context() {
            return Err("spells cannot reference triggering-spell mana spending".into());
        }
        if context == EffectContext::Ability {
            self.validate_cast_snapshot_references(0)?;
        }
        for filter in self.target_filters() {
            filter.validate_target_constraints()?;
        }
        if let SpellEffectKind::CounterTargetSpell { spell_filter, .. }
        | SpellEffectKind::CopyTargetSpell { spell_filter, .. } = self
        {
            spell_filter.validate()?;
        }
        if let SpellEffectKind::MoveGraveyardCards { filter, .. } = self {
            filter.validate()?;
        }
        if let SpellEffectKind::PutCounters { counter, .. }
        | SpellEffectKind::RemoveCounters { counter, .. } = self
        {
            counter.validate()?;
        }
        if matches!(self, SpellEffectKind::PutCounterSnapshot { .. })
            && context == EffectContext::Spell
        {
            return Err("counter snapshots require a departure ability".into());
        }
        if matches!(self, SpellEffectKind::RemoveCounters { count: 0, .. }) {
            return Err("counter removal requires a positive count".into());
        }
        if matches!(self, SpellEffectKind::Blight { count: 0 }) {
            return Err("Blight requires a positive counter count".into());
        }
        if let SpellEffectKind::CreateTokenCopies { target, .. } = self {
            if !target.is_permanent_only() {
                return Err("CreateTokenCopies requires a permanent source".into());
            }
        }
        if let SpellEffectKind::Exile { subject }
        | SpellEffectKind::PutInOwnersLibrary { subject, .. } = self
        {
            if let EffectSubject::Chosen(target) = subject {
                if !target.is_permanent_only() {
                    return Err("zone-move subjects require a permanent target".into());
                }
            }
        }
        if let SpellEffectKind::AttachSource { target } = self {
            if !target.is_permanent_only() {
                return Err("AttachSource requires a permanent target".into());
            }
        }

        match self {
            SpellEffectKind::Conditional { condition, effect } => {
                condition.validate()?;
                if matches!(effect.as_ref(), SpellEffectKind::Conditional { .. }) {
                    return Err("Conditional effects cannot be nested".into());
                }
                if !matches!(
                    effect.as_ref(),
                    SpellEffectKind::Destroy { .. }
                        | SpellEffectKind::GrantKeywords { .. }
                        | SpellEffectKind::ChoosePermanents { .. }
                        | SpellEffectKind::Draw { .. }
                ) {
                    return Err(
                        "Conditional currently supports Destroy, GrantKeywords, ChoosePermanents, and Draw effects"
                            .into(),
                    );
                }
                effect.validate(context)?;
            }
            SpellEffectKind::ConditionalCastCost { effect, .. } => {
                if matches!(effect.as_ref(), SpellEffectKind::ConditionalCastCost { .. }) {
                    return Err("ConditionalCastCost effects cannot be nested".into());
                }
                if !matches!(
                    effect.as_ref(),
                    SpellEffectKind::PumpTarget { .. } | SpellEffectKind::GainLife { .. }
                ) {
                    return Err(
                        "ConditionalCastCost currently supports PumpTarget and GainLife effects"
                            .into(),
                    );
                }
                effect.validate(context)?;
            }
            SpellEffectKind::DamageTarget { amount, .. }
            | SpellEffectKind::DamagePlayer { amount, .. }
            | SpellEffectKind::DamageAll { amount, .. }
            | SpellEffectKind::Scry { count: amount }
            | SpellEffectKind::Earthbend { count: amount }
            | SpellEffectKind::CounterTargetSpell {
                unless_controller_pays: Some(amount),
                ..
            }
            | SpellEffectKind::Draw { count: amount, .. }
            | SpellEffectKind::GainLife { amount }
            | SpellEffectKind::Mill { count: amount, .. }
            | SpellEffectKind::PutCounters { count: amount, .. }
            | SpellEffectKind::Amass { count: amount, .. }
            | SpellEffectKind::CreateTokens { count: amount, .. }
            | SpellEffectKind::CreateTokenCopies { count: amount, .. }
            | SpellEffectKind::CreateAttackingTokens { count: amount, .. } => {
                amount.validate_effect(context)?
            }
            SpellEffectKind::PumpTarget {
                scale: Some(scale), ..
            } => scale.validate_effect(context)?,
            SpellEffectKind::DamageTargets { amount, .. } => {
                amount.validate()?;
                if amount.requires_game_state() {
                    return Err(
                        "DamageTargets cannot use a game-state amount because its allocation is chosen at cast time"
                            .into(),
                    );
                }
            }
            SpellEffectKind::CreatureDealsDamageEqualToPower { source, target }
                if !source.all_terminal_filters_match(|leaf| leaf.kind == TargetKind::Creature)
                    || !target
                        .all_terminal_filters_match(|leaf| leaf.kind == TargetKind::Creature) =>
            {
                return Err(
                    "CreatureDealsDamageEqualToPower requires two creature target filters".into(),
                );
            }
            SpellEffectKind::SetBasePowerToughness {
                target,
                power,
                toughness,
            } => {
                if !target.all_terminal_filters_match(|leaf| {
                    matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                }) {
                    return Err(
                        "SetBasePowerToughness requires a battlefield-permanent target".into(),
                    );
                }
                if context == EffectContext::Spell
                    && (matches!(power, BasePowerToughnessValue::Source(_))
                        || matches!(toughness, BasePowerToughnessValue::Source(_)))
                {
                    return Err(
                        "source-relative base P/T values require a permanent ability source".into(),
                    );
                }
            }
            SpellEffectKind::Fight { first, second } => {
                for subject in [first, second] {
                    if let EffectSubject::Chosen(filter) = subject {
                        if !filter
                            .all_terminal_filters_match(|leaf| leaf.kind == TargetKind::Creature)
                        {
                            return Err(
                                "Fight chosen subjects require creature target filters".into()
                            );
                        }
                    }
                }
            }
            SpellEffectKind::PreventAllCombatDamageToTargetTurn { target }
                if !target.all_terminal_filters_match(|leaf| leaf.kind == TargetKind::Creature) =>
            {
                return Err(
                    "PreventAllCombatDamageToTargetTurn requires a creature target filter".into(),
                );
            }
            SpellEffectKind::DestroyAttached {
                target,
                attachments,
            } => {
                if !target.is_permanent_only() {
                    return Err(format!(
                        "DestroyAttached cannot target players, got {:?}",
                        target.kind
                    ));
                }
                attachments.validate()?;
            }
            SpellEffectKind::PumpAll { filter, .. }
            | SpellEffectKind::GrantKeywordsAll { filter, .. }
            | SpellEffectKind::RemoveAbilitiesAll { filter } => filter.validate()?,
            SpellEffectKind::ReturnTriggeredCard {
                from,
                entry_counters,
                ..
            } => {
                if from.is_empty()
                    || from
                        .iter()
                        .any(|zone| !matches!(zone, EventZone::Graveyard | EventZone::Exile))
                {
                    return Err(
                        "ReturnTriggeredCard requires graveyard and/or exile source zones".into(),
                    );
                }
                let mut kinds = std::collections::HashSet::new();
                for placement in entry_counters {
                    placement.counter.validate()?;
                    if placement.count == 0 {
                        return Err("entry counter placement count must be positive".into());
                    }
                    if !kinds.insert(placement.counter) {
                        return Err("entry counter placements cannot repeat a counter kind".into());
                    }
                }
            }
            SpellEffectKind::ChoosePermanents {
                chooser,
                filter,
                min,
                max,
                constraints,
            } => {
                if matches!(
                    chooser,
                    PlayerRecipient::EachOpponent | PlayerRecipient::EachPlayer
                ) {
                    return Err("permanent choice requires exactly one deciding player".into());
                }
                if *max == 0 || min > max {
                    return Err("permanent choice requires 0 <= min <= max and max > 0".into());
                }
                filter.validate_target_constraints()?;
                if !filter.all_terminal_filters_match(|leaf| {
                    matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                }) {
                    return Err("permanent choice requires permanent-only filters".into());
                }
                if !constraints.is_empty()
                    && !filter.all_terminal_filters_match(|leaf| {
                        matches!(leaf.kind, TargetKind::AnyPermanent)
                            && leaf
                                .required_subtypes
                                .iter()
                                .any(|subtype| subtype == "Equipment")
                    })
                {
                    return Err(
                        "Equipment attachment choice constraints require an Equipment-only filter"
                            .into(),
                    );
                }
            }
            SpellEffectKind::ChooseResolutionBranch {
                chooser,
                optional,
                selection,
                branches,
            } => {
                if branches.is_empty() {
                    return Err("resolution choice requires at least one branch".into());
                }
                if matches!(
                    chooser,
                    PlayerRecipient::EachOpponent | PlayerRecipient::EachPlayer
                ) {
                    return Err("resolution choice requires exactly one deciding player".into());
                }
                let mut branch_ids = std::collections::HashSet::new();
                for (branch_index, branch) in branches.iter().enumerate() {
                    branch.branch_id.validate()?;
                    branch.presentation.validate()?;
                    if !branch_ids.insert(branch.branch_id.as_str()) {
                        return Err(format!(
                            "duplicate resolution branch id '{}'",
                            branch.branch_id
                        ));
                    }
                    let is_first_applicable_noop_fallback = *selection
                        == ResolutionBranchSelection::FirstApplicable
                        && branch_index + 1 == branches.len()
                        && matches!(branch.requirement, ResolutionBranchRequirement::Always)
                        && branch.cost == ResolutionCost::None;
                    // Paying a real cost can be the entire successful branch (Command Bridge,
                    // Transguild Promenade); it needs no fabricated follow-up effect.
                    let is_cost_only_branch = branch.cost != ResolutionCost::None;
                    if branch.effects.is_empty()
                        && !is_first_applicable_noop_fallback
                        && !is_cost_only_branch
                    {
                        return Err(
                            "resolution choice branches require an effect or payment".into()
                        );
                    }
                    match &branch.cost {
                        ResolutionCost::Blight { count } => {
                            if *count == 0 {
                                return Err("blight cost requires a positive count".into());
                            }
                        }
                        ResolutionCost::None => {}
                        ResolutionCost::Mana(cost) | ResolutionCost::Waterbend(cost) => {
                            if cost.pips.is_empty()
                                || cost
                                    .pips
                                    .iter()
                                    .any(|pip| matches!(pip, crate::ManaSymbol::X))
                            {
                                return Err(
                                    "resolution mana cost must be nonempty and cannot contain X"
                                        .into(),
                                );
                            }
                        }
                        ResolutionCost::DiscardCard { .. } => {}
                        ResolutionCost::SacrificePermanent {
                            filter,
                            source_only,
                        } => {
                            filter.validate_target_constraints()?;
                            if *source_only
                                && filter.any_terminal_filter_matches(|leaf| {
                                    !leaf.excluded_objects.is_empty()
                                })
                            {
                                return Err(
                                    "source-only sacrifice cannot exclude its source".into()
                                );
                            }
                            if !filter.all_terminal_filters_match(|leaf| {
                                matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                                    && leaf.controller == TargetController::You
                            }) {
                                return Err(
                                    "resolution sacrifice cost must select a permanent you control"
                                        .into(),
                                );
                            }
                        }
                        ResolutionCost::TapPermanents { count, filter, .. } => {
                            if *count == 0 {
                                return Err("resolution tap cost requires a positive count".into());
                            }
                            filter.validate_target_constraints()?;
                            if !filter.all_terminal_filters_match(|leaf| {
                                matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                                    && leaf.controller == TargetController::You
                            }) {
                                return Err(
                                    "resolution tap cost must select permanents you control".into(),
                                );
                            }
                        }
                    }
                    if let ResolutionBranchRequirement::GameCondition(condition) =
                        &branch.requirement
                    {
                        condition.validate()?;
                    }
                    if let ResolutionBranchRequirement::CardResultCount { min, max, .. } =
                        &branch.requirement
                    {
                        if min.is_none() && max.is_none() {
                            return Err("card result count requires a minimum or maximum".into());
                        }
                        if min
                            .zip(*max)
                            .is_some_and(|(minimum, maximum)| minimum > maximum)
                        {
                            return Err("card result count minimum cannot exceed maximum".into());
                        }
                    }
                    for effect in &branch.effects {
                        if effect.needs_target() {
                            return Err(
                                "resolution branch effects cannot target directly; create a reflexive trigger instead"
                                    .into(),
                            );
                        }
                        if matches!(effect, SpellEffectKind::ChooseResolutionBranch { .. }) {
                            return Err("nested resolution branch choices are not supported".into());
                        }
                        effect.validate(context)?;
                    }
                    if matches!(
                        branch.requirement,
                        ResolutionBranchRequirement::EffectsApplicable
                    ) && !branch
                        .effects
                        .iter()
                        .any(|effect| matches!(effect, SpellEffectKind::PutCounters { .. }))
                    {
                        return Err(
                            "EffectsApplicable requires a supported applicability-sensitive effect"
                                .into(),
                        );
                    }
                    SpellEffectKind::validate_list(&branch.effects)?;
                }
                if !optional
                    && branches.iter().any(|branch| {
                        !matches!(branch.requirement, ResolutionBranchRequirement::Always)
                    })
                    && !branches.iter().any(|branch| {
                        matches!(branch.requirement, ResolutionBranchRequirement::Always)
                            && branch.cost == ResolutionCost::None
                    })
                {
                    return Err(
                        "mandatory condition-gated resolution choice requires an unconditional costless fallback"
                            .into(),
                    );
                }
                if *selection == ResolutionBranchSelection::FirstApplicable {
                    if *optional || *chooser != PlayerRecipient::Controller {
                        return Err(
                            "FirstApplicable resolution branches must be mandatory and controller-relative"
                                .into(),
                        );
                    }
                    if branches
                        .iter()
                        .any(|branch| branch.cost != ResolutionCost::None)
                    {
                        return Err("FirstApplicable resolution branches must be costless".into());
                    }
                    if !branches.last().is_some_and(|branch| {
                        matches!(branch.requirement, ResolutionBranchRequirement::Always)
                    }) || branches[..branches.len() - 1].iter().any(|branch| {
                        matches!(branch.requirement, ResolutionBranchRequirement::Always)
                    }) {
                        return Err(
                            "FirstApplicable resolution branches require exactly one final unconditional fallback"
                                .into(),
                        );
                    }
                }
            }
            SpellEffectKind::CreateReflexiveTrigger { ability } => {
                ability.validate_shape()?;
            }
            _ => {}
        }

        if let SpellEffectKind::DrawDiscard {
            who,
            draw_count,
            discard_count,
            order,
            optional,
            ..
        } = self
        {
            if *draw_count == 0 || *discard_count == 0 {
                return Err("DrawDiscard counts must be at least 1".into());
            }
            if *optional && (*order != DrawDiscardOrder::DiscardThenDraw || *discard_count != 1) {
                return Err(
                    "optional DrawDiscard requires DiscardThenDraw with discard_count 1".into(),
                );
            }
            if matches!(
                who,
                PlayerRecipient::EachOpponent
                    | PlayerRecipient::EachPlayer
                    | PlayerRecipient::AttackingOpponentsOfDefendingPlayer
            ) {
                return Err("DrawDiscard requires a single player recipient".into());
            }
        }

        if let SpellEffectKind::Discard { count: 0, .. } = self {
            return Err("Discard count must be at least 1".into());
        }

        // CR 115: a source-bound ability effect is not targeting and only exists where there is
        // a source permanent — never in a spell's effect list.
        let source_bound = matches!(
            self,
            SpellEffectKind::PumpTarget {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::PutCounters {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::RemoveCounters {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::PutCounterSnapshot {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::GrantKeywordChoice {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::GrantTriggeredAbility {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::CreateDelayedTrigger {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::AddTypes {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::Regenerate {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
            } | SpellEffectKind::Untap {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
            } | SpellEffectKind::Tap {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
            } | SpellEffectKind::Fight {
                first: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::Fight {
                second: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::ReturnToOwnersHand {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
            } | SpellEffectKind::Exile {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
            } | SpellEffectKind::PutInOwnersLibrary {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::Destroy {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
            } | SpellEffectKind::AttachSource { .. }
                | SpellEffectKind::ChangeSourceFace { .. }
                | SpellEffectKind::ReturnTriggeredCard { .. }
                | SpellEffectKind::ApplyCombatRestriction {
                    scope: CombatRestrictionScope::Source,
                    ..
                }
                | SpellEffectKind::LoseLife {
                    who: PlayerRecipient::TriggerObjectController,
                    ..
                }
                | SpellEffectKind::DamagePlayer {
                    who: PlayerRecipient::TriggerObjectController,
                    ..
                }
                | SpellEffectKind::Mill {
                    who: PlayerRecipient::TriggerObjectController,
                    ..
                }
                | SpellEffectKind::DrawDiscard {
                    who: PlayerRecipient::TriggerObjectController,
                    ..
                }
        );
        if context == EffectContext::Spell && source_bound {
            return Err(
                "source-bound effects are only valid on an activated or triggered ability, not a spell"
                    .into(),
            );
        }
        match self {
            SpellEffectKind::TargetPlayerGainsLife { target, .. }
            | SpellEffectKind::TargetPlayerLosesLife { target, .. }
            | SpellEffectKind::DrainTarget { target, .. }
            | SpellEffectKind::MillTargetPlayer { target, .. }
            | SpellEffectKind::DiscardCards { target, .. }
            | SpellEffectKind::ExileCardsFromHand { target, .. } => {
                if target.is_player() {
                    Ok(())
                } else {
                    Err(format!(
                        "player-targeted effect requires AnyPlayer or OpponentPlayer kind, got {:?}",
                        target.kind
                    ))
                }
            }
            // CR 701.19/701.20: tapping and chosen-subject untapping act on permanents, never
            // players. A source subject is already constrained to a permanent ability above.
            SpellEffectKind::SkipNextUntap { target }
            | SpellEffectKind::GainControlUntilEndOfTurn { target } => {
                if !target.is_permanent_only() {
                    Err(format!(
                        "permanent effect cannot target players, got {:?}",
                        target.kind
                    ))
                } else {
                    Ok(())
                }
            }
            SpellEffectKind::Tap {
                subject: EffectSubject::Chosen(target),
            }
            | SpellEffectKind::Untap {
                subject: EffectSubject::Chosen(target),
            }
            | SpellEffectKind::ReturnToOwnersHand {
                subject: EffectSubject::Chosen(target),
            }
            | SpellEffectKind::Exile {
                subject: EffectSubject::Chosen(target),
            }
            | SpellEffectKind::PutInOwnersLibrary {
                subject: EffectSubject::Chosen(target),
                ..
            }
            | SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(target),
            } => {
                if !target.is_permanent_only() {
                    Err(format!(
                        "permanent effect cannot target players, got {:?}",
                        target.kind
                    ))
                } else {
                    Ok(())
                }
            }
            // CR 122: counters go on permanents, never players.
            SpellEffectKind::PutCounters {
                subject: EffectSubject::Chosen(target),
                ..
            }
            | SpellEffectKind::RemoveCounters {
                subject: EffectSubject::Chosen(target),
                ..
            }
            | SpellEffectKind::PutCounterSnapshot {
                subject: EffectSubject::Chosen(target),
                ..
            } => {
                if !target.is_permanent_only() {
                    Err(format!(
                        "PutCounters cannot target players, got {:?}",
                        target.kind
                    ))
                } else {
                    Ok(())
                }
            }
            // TargetPlayerSacrifices targets a player (kind must be AnyPlayer/OpponentPlayer),
            // and the sacrifice filter must select objects not players.
            SpellEffectKind::TargetPlayerSacrifices { target, filter } => {
                filter.validate_characteristic_constraints()?;
                if !target.is_player() {
                    return Err(format!(
                        "TargetPlayerSacrifices.target must be AnyPlayer or OpponentPlayer, got {:?}",
                        target.kind
                    ));
                }
                if !filter.is_permanent_only() {
                    return Err(format!(
                        "TargetPlayerSacrifices.filter must select permanents, not players, got {:?}",
                        filter.kind
                    ));
                }
                if !filter
                    .all_terminal_filters_match(|leaf| leaf.controller == TargetController::Any)
                {
                    return Err(
                        "TargetPlayerSacrifices.filter cannot use a controller-relative target filter"
                            .into(),
                    );
                }
                if filter.any_terminal_filter_matches(|leaf| !leaf.excluded_objects.is_empty()) {
                    return Err(
                        "TargetPlayerSacrifices.filter cannot exclude the effect source".into(),
                    );
                }
                Ok(())
            }
            // Mass effects select objects, not players, and never use AnyTarget (which includes
            // players). Only Creature / AnyPermanent are honored by the engine.
            SpellEffectKind::DestroyAll { kind, .. }
            | SpellEffectKind::DamageAll { kind, .. }
            | SpellEffectKind::UntapAll { filter: kind, .. } => {
                kind.validate_characteristic_constraints()?;
                if !kind.all_terminal_filters_match(|leaf| {
                    matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                }) {
                    return Err(format!(
                        "mass effect kind must be Creature or AnyPermanent, got {:?}",
                        kind.kind
                    ));
                }
                // Untargeted mass selection runs through `battlefield_objects_matching`, which has
                // no activating player to compare a controller against — controller scope belongs
                // in the effect's own `players` (RelativePlayerSet) or `CreatureScopeFilter`, not here.
                // Rejecting it beats silently ignoring it.
                if !kind.all_terminal_filters_match(|leaf| leaf.controller == TargetController::Any)
                {
                    return Err(
                        "mass effect filter cannot use a controller relationship; scope the effect with \
                         `players` (RelativePlayerSet) instead"
                            .into(),
                    );
                }
                if kind.any_terminal_filter_matches(|leaf| !leaf.excluded_objects.is_empty()) {
                    return Err("mass effect filter cannot exclude the effect source".into());
                }
                Ok(())
            }
            SpellEffectKind::GrantKeywordChoice { subject, choices } => {
                if choices.len() < 2
                    || choices
                        .iter()
                        .enumerate()
                        .any(|(i, kw)| choices[..i].contains(kw))
                {
                    return Err("keyword choice requires at least two distinct keywords".into());
                }
                SpellEffectKind::GrantKeywords {
                    subject: subject.clone(),
                    keywords: choices.clone(),
                }
                .validate(context)
            }
            SpellEffectKind::GrantKeywords { subject, keywords } => {
                if let EffectSubject::Chosen(target) = subject {
                    if !target.is_permanent_only() {
                        return Err(format!(
                            "GrantKeywords cannot target players, got {:?}",
                            target.kind
                        ));
                    }
                }
                if keywords.is_empty() {
                    Err("GrantKeywords requires at least one keyword".into())
                } else {
                    Ok(())
                }
            }
            SpellEffectKind::GrantProtection {
                subject,
                protection,
            } => {
                if let EffectSubject::Chosen(target) = subject {
                    if !target.is_permanent_only() {
                        return Err(format!(
                            "GrantProtection cannot target players, got {:?}",
                            target.kind
                        ));
                    }
                }
                if let ProtectionGrant::Choose(options) = protection {
                    if options.is_empty() {
                        return Err("protection choice requires at least one quality".into());
                    }
                    let unique = options
                        .iter()
                        .copied()
                        .collect::<std::collections::HashSet<_>>();
                    if unique.len() != options.len() {
                        return Err("protection choice repeats a quality".into());
                    }
                }
                Ok(())
            }
            SpellEffectKind::GrantTriggeredAbility { subject, ability } => {
                if let EffectSubject::Chosen(target) = subject {
                    if !target.all_terminal_filters_match(|leaf| {
                        matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                    }) {
                        return Err(format!(
                            "GrantTriggeredAbility requires a permanent-only target, got {:?}",
                            target.kind
                        ));
                    }
                }
                if ability.trigger.is_delayed_only() {
                    return Err(
                        "GrantTriggeredAbility cannot use a delayed trigger condition".into(),
                    );
                }
                if matches!(ability.trigger, TriggerCondition::SagaChapter { .. }) {
                    return Err(
                        "Saga chapter triggers must be printed on an Enchantment Saga face".into(),
                    );
                }
                ability.validate_shape()
            }
            SpellEffectKind::CreateDelayedTrigger { subject, ability } => {
                if let EffectSubject::Chosen(target) = subject {
                    if !target.all_terminal_filters_match(|leaf| {
                        matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                    }) {
                        return Err(format!(
                            "CreateDelayedTrigger requires a permanent-only target, got {:?}",
                            target.kind
                        ));
                    }
                }
                if !ability.trigger.is_delayed_only() {
                    return Err("CreateDelayedTrigger requires a delayed trigger condition".into());
                }
                ability.validate_shape()
            }
            SpellEffectKind::AddTypes { subject, addition } => {
                if let EffectSubject::Chosen(target) = subject {
                    if !target.all_terminal_filters_match(|leaf| {
                        matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                    }) {
                        return Err(format!(
                            "AddTypes requires a permanent-only target, got {:?}",
                            target.kind
                        ));
                    }
                    if !addition.creature_types.is_empty()
                        && target.any_terminal_filter_matches(|leaf| {
                            leaf.kind == TargetKind::AnyPermanent
                        })
                        && !addition
                            .card_types
                            .contains(&super::PermanentTypeFilter::Creature)
                    {
                        return Err(
                            "AddTypes creature types require a Creature target or adding Creature"
                                .into(),
                        );
                    }
                }
                addition.validate()
            }
            SpellEffectKind::GrantKeywordsAllPermanents { filter, keywords } => {
                filter.validate_characteristic_constraints()?;
                if !filter.all_terminal_filters_match(|leaf| {
                    matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                }) {
                    Err(format!(
                        "GrantKeywordsAllPermanents filter must be Creature or AnyPermanent, got {:?}",
                        filter.kind
                    ))
                } else if filter.any_terminal_filter_matches(|leaf| {
                    leaf.controller == TargetController::Opponent
                }) {
                    Err(
                        "GrantKeywordsAllPermanents does not support opponent scope; use the dedicated untargeted player scope"
                            .into(),
                    )
                } else if filter
                    .any_terminal_filter_matches(|leaf| !leaf.excluded_objects.is_empty())
                {
                    Err("GrantKeywordsAllPermanents filter cannot exclude the effect source".into())
                } else if keywords.is_empty() {
                    Err("GrantKeywordsAllPermanents requires at least one keyword".into())
                } else {
                    Ok(())
                }
            }
            SpellEffectKind::ApplyCombatRestriction { scope, restriction } => {
                restriction.validate()?;
                match scope {
                    CombatRestrictionScope::Source => Ok(()),
                    CombatRestrictionScope::Chosen(target) => {
                        if !target.is_permanent_only() {
                            Err(format!(
                                "ApplyCombatRestriction cannot target players, got {:?}",
                                target.kind
                            ))
                        } else {
                            Ok(())
                        }
                    }
                    CombatRestrictionScope::Matching(filter) => {
                        filter.validate_characteristic_constraints()?;
                        if filter.any_terminal_filter_matches(|leaf| {
                            leaf.excluded_objects
                                .contains(&super::TargetObjectExclusion::AttachedObject)
                        }) {
                            return Err(
                                "matching combat restrictions do not bind attachment identity"
                                    .into(),
                            );
                        }
                        if !filter
                            .all_terminal_filters_match(|leaf| leaf.kind == TargetKind::Creature)
                        {
                            Err(format!(
                                "matching combat restriction requires Creature kind, got {:?}",
                                filter.kind
                            ))
                        } else {
                            Ok(())
                        }
                    }
                }
            }
            // CR 605.1a: a mana ability is an activated/triggered ability — never a spell. An
            // empty option set would produce nothing and is rejected as malformed.
            SpellEffectKind::ProduceMana {
                options,
                restriction,
                conditional,
            } => {
                if context == EffectContext::Spell {
                    return Err("ProduceMana is only valid on a mana ability, not a spell".into());
                }
                if options.is_empty() {
                    return Err("ProduceMana requires at least one mana option".into());
                }
                if let Some(restriction) = restriction {
                    restriction.validate()?;
                }
                if let Some(conditional) = conditional {
                    conditional.condition.validate()?;
                    if conditional.options.is_empty() {
                        return Err(
                            "conditional ProduceMana requires at least one mana option".into()
                        );
                    }
                }
                Ok(())
            }
            SpellEffectKind::AddMana { amount, .. }
                if [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
                    .into_iter()
                    .all(|count| count == 0) =>
            {
                Err("AddMana requires at least one mana".into())
            }
            SpellEffectKind::CounterTargetSpell {
                unless_controller_pays: Some(_),
                unless_controller_pays_by_cast_cost: Some(_),
                ..
            } => Err("CounterTargetSpell payment forms are mutually exclusive".into()),
            SpellEffectKind::CounterTargetSpell {
                unless_controller_pays_by_cast_cost: Some(conditional),
                ..
            } if conditional.if_selected == 0 || conditional.otherwise == 0 => {
                Err("receipt-conditioned CounterTargetSpell payments must be at least 1".into())
            }
            SpellEffectKind::CounterTargetSpell {
                unless_controller_pays: Some(Amount::Fixed(0)),
                ..
            } => Err("CounterTargetSpell unless_controller_pays must be at least 1".into()),
            SpellEffectKind::CounterTriggeringStackObjectUnlessPays { cost } => match cost {
                ResolutionCost::Mana(cost)
                    if !cost.pips.is_empty()
                        && !cost
                            .pips
                            .iter()
                            .any(|pip| matches!(pip, crate::ManaSymbol::X)) =>
                {
                    Ok(())
                }
                ResolutionCost::DiscardCard { .. } => Ok(()),
                ResolutionCost::Mana(_) => {
                    Err("Ward mana cost must be nonempty and cannot contain X".into())
                }
                ResolutionCost::Waterbend(_)
                | ResolutionCost::None
                | ResolutionCost::Blight { .. }
                | ResolutionCost::SacrificePermanent { .. }
                | ResolutionCost::TapPermanents { .. } => {
                    Err("Ward supports only mana and discard-card costs".into())
                }
            },
            // Library searches use the resolution-interrupt machinery and are legal on spells
            // and nonmana abilities alike (Demonic Tutor, Evolving Wilds).
            SpellEffectKind::SearchLibrary {
                count,
                filter,
                slots,
                zones,
                destination,
                conditional_destination,
                count_by_cast_cost,
                ..
            } => {
                if slots.is_empty() && *count == 0 {
                    return Err("SearchLibrary requires a positive count".into());
                }
                if !slots.is_empty() && (filter.is_some() || count_by_cast_cost.is_some()) {
                    return Err(
                        "SearchLibrary heterogeneous slots cannot combine with filter or count_by_cast_cost"
                            .into(),
                    );
                }
                if count_by_cast_cost.as_ref().is_some_and(|conditional| {
                    conditional.if_selected == 0 || conditional.otherwise == 0
                }) {
                    return Err("SearchLibrary cast-cost counts must be positive".into());
                }
                if let Some(filter) = filter {
                    filter.validate()?;
                }
                let mut slot_ids = std::collections::HashSet::new();
                for slot in slots {
                    slot.slot_id.validate()?;
                    slot.presentation.validate()?;
                    if !slot_ids.insert(slot.slot_id.as_str()) {
                        return Err(format!(
                            "duplicate SearchLibrary slot id '{}'",
                            slot.slot_id
                        ));
                    }
                    slot.filter.validate()?;
                }
                zones.validate()?;
                if !slots.is_empty()
                    && !matches!(zones, SearchZoneSelection::Fixed(zones) if zones == &[CardSearchZone::Library])
                {
                    return Err(
                        "SearchLibrary heterogeneous slots currently require the controller's library"
                            .into(),
                    );
                }
                if !slots.is_empty()
                    && (*destination != SearchDestination::Hand
                        || conditional_destination.is_some())
                {
                    return Err(
                        "SearchLibrary heterogeneous slots currently require an unconditional Hand destination"
                            .into(),
                    );
                }
                if let Some(conditional) = conditional_destination {
                    conditional.condition.validate()?;
                }
                Ok(())
            }
            SpellEffectKind::ExileTopWithPlayPermission {
                count,
                count_by_cast_cost,
                ..
            } => {
                if *count == 0 {
                    return Err("ExileTopWithPlayPermission requires a positive count".into());
                }
                if count_by_cast_cost.as_ref().is_some_and(|conditional| {
                    conditional.if_selected == 0 || conditional.otherwise == 0
                }) {
                    return Err(
                        "ExileTopWithPlayPermission cast-cost counts must be positive".into(),
                    );
                }
                Ok(())
            }
            SpellEffectKind::LookChooseToHand { count, filter, .. } => {
                if *count == 0 {
                    return Err("LookChooseToHand requires a positive count".into());
                }
                filter.validate()
            }
            SpellEffectKind::ChooseGraveyardCard {
                filter,
                from_result,
                ..
            } => {
                filter.validate()?;
                if from_result.as_ref().is_some_and(|result| {
                    result.source != CardResultSource::PreviousEffect
                        || result.action != CardResultAction::Mill
                }) {
                    return Err(
                        "ChooseGraveyardCard supports only a previous Mill result cohort".into(),
                    );
                }
                Ok(())
            }
            // CR 701.18: scry is legal on spells and on abilities alike (scry lands, Sensei's
            // Divining Top-style activations). Reject a useless literal zero; a dynamic amount
            // may evaluate to zero and then does nothing.
            SpellEffectKind::Scry { count } => {
                if matches!(count, Amount::Fixed(0)) {
                    Err("Scry requires a count of at least 1".into())
                } else {
                    Ok(())
                }
            }
            SpellEffectKind::LibraryPartition {
                count,
                top_min,
                top_max,
                kind,
            } => {
                if *count == 0 {
                    return Err("LibraryPartition requires a count of at least 1".into());
                }
                if *top_min > *count {
                    return Err("LibraryPartition top_min cannot exceed count".into());
                }
                if top_max.is_some_and(|maximum| maximum < *top_min || maximum > *count) {
                    return Err("LibraryPartition top_max must be between top_min and count".into());
                }
                if *kind == LibraryPartitionKind::Surveil && (*top_min != 0 || top_max.is_some()) {
                    return Err(
                        "Surveil LibraryPartition must allow any number of cards on top".into(),
                    );
                }
                Ok(())
            }
            SpellEffectKind::ManifestDread => Ok(()),
            // CR 303.4a: an Aura may enchant an object or player. Keep mixed AnyTarget out until
            // a real card needs the additional recipient disambiguation surface.
            SpellEffectKind::AuraAttach { target } => {
                let object_only = target.all_terminal_filters_match(|leaf| {
                    matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                });
                let player_only = target.all_terminal_filters_match(|leaf| {
                    matches!(
                        leaf.kind,
                        TargetKind::AnyPlayer | TargetKind::OpponentPlayer
                    )
                });
                if !object_only && !player_only {
                    Err("AuraAttach does not support mixed AnyTarget recipients".into())
                } else {
                    Ok(())
                }
            }
            SpellEffectKind::AttachSource { target } => {
                if context == EffectContext::Spell {
                    Err("AttachSource is only valid on an activated or triggered ability".into())
                } else if !target.is_permanent_only() {
                    Err(format!(
                        "AttachSource cannot target players, got {:?}",
                        target.kind
                    ))
                } else {
                    Ok(())
                }
            }
            SpellEffectKind::AttachEquipment {
                equipment,
                creature,
            } => {
                match equipment {
                    EffectSubject::Chosen(filter) => {
                        if !filter.all_terminal_filters_match(|leaf| {
                            leaf.kind == TargetKind::AnyPermanent
                                && leaf
                                    .required_subtypes
                                    .iter()
                                    .any(|subtype| subtype == "Equipment")
                        }) {
                            return Err(
                                "AttachEquipment equipment target must require Equipment".into()
                            );
                        }
                    }
                    EffectSubject::PreviousEffectObject => {}
                    _ => {
                        return Err(
                            "AttachEquipment equipment must be Chosen or PreviousEffectObject"
                                .into(),
                        );
                    }
                }
                match creature {
                    EffectSubject::Chosen(filter)
                        if filter.all_terminal_filters_match(|leaf| {
                            leaf.kind == TargetKind::Creature
                        }) => {}
                    _ => {
                        return Err(
                            "AttachEquipment creature must be a creature-only Chosen target".into(),
                        );
                    }
                }
                Ok(())
            }
            // CR 702.6a: equip is an activated ability that only attaches to creatures you
            // control — never a spell effect, and the filter must be creature-typed.
            SpellEffectKind::Equip { target } => {
                if context == EffectContext::Spell {
                    Err("Equip is only valid on an activated ability, not a spell".into())
                } else if !target.is_permanent_only() {
                    Err(format!(
                        "Equip cannot target players, got {:?}",
                        target.kind
                    ))
                } else {
                    Ok(())
                }
            }
            // CR 701.19: Regenerate puts a shield on the target; it is an activated ability, not
            // a spell. Applying a regeneration shield via a spell would have no source permanent
            // to attach the replacement to and is a nonsensical card design — reject early.
            SpellEffectKind::Regenerate { .. } => {
                if context == EffectContext::Spell {
                    Err("Regenerate is only valid on an activated or triggered ability, not a spell"
                        .into())
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn requires_triggering_spell_context(&self) -> bool {
        match self {
            Self::DamageTarget { amount, .. }
            | Self::DamageAll { amount, .. }
            | Self::Scry { count: amount }
            | Self::Earthbend { count: amount }
            | Self::CounterTargetSpell {
                unless_controller_pays: Some(amount),
                ..
            }
            | Self::DamageTargets { amount, .. }
            | Self::DamagePlayer { amount, .. }
            | Self::DamageAttackedPlayerOrPlaneswalker { amount }
            | Self::Draw { count: amount, .. }
            | Self::GainLife { amount }
            | Self::Mill { count: amount, .. }
            | Self::PutCounters { count: amount, .. }
            | Self::Amass { count: amount, .. }
            | Self::CreateTokens { count: amount, .. }
            | Self::CreateTokenCopies { count: amount, .. }
            | Self::CreateAttackingTokens { count: amount, .. } => {
                amount.requires_triggering_spell_context()
            }
            Self::PumpTarget {
                scale: Some(scale), ..
            } => scale.requires_triggering_spell_context(),
            Self::ChooseResolutionBranch { branches, .. } => branches.iter().any(|branch| {
                matches!(
                    &branch.requirement,
                    ResolutionBranchRequirement::GameCondition(condition)
                        if condition.requires_triggering_spell_context()
                ) || branch
                    .effects
                    .iter()
                    .any(Self::requires_triggering_spell_context)
            }),
            Self::Conditional { condition, effect } => {
                condition.requires_triggering_spell_context()
                    || effect.requires_triggering_spell_context()
            }
            Self::SearchLibrary {
                conditional_destination: Some(conditional),
                ..
            } => conditional.condition.requires_triggering_spell_context(),
            Self::ProduceMana {
                conditional: Some(conditional),
                ..
            } => conditional.condition.requires_triggering_spell_context(),
            _ => false,
        }
    }
}

/// Serde default for `CopyTargetSpell.count` — the overwhelmingly common "make one copy".
fn one() -> u32 {
    1
}

#[cfg(test)]
mod attachment_filter_tests {
    use super::*;

    #[test]
    fn attachment_filter_requires_unique_nonempty_kinds() {
        assert!(AttachmentFilter { kinds: vec![] }.validate().is_err());
        assert!(AttachmentFilter {
            kinds: vec![AttachmentKind::Aura, AttachmentKind::Aura],
        }
        .validate()
        .is_err());
        assert!(AttachmentFilter {
            kinds: vec![AttachmentKind::Aura, AttachmentKind::Equipment],
        }
        .validate()
        .is_ok());
    }
}

// ---------------------------------------------------------------------------
// Continuous effects (layer system, CR 613)
// ---------------------------------------------------------------------------

/// How long a continuous effect lasts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectDuration {
    /// CR 611.2a: no stated duration. Earthbend from Badgermole and Rebellious Captives
    /// lasts across turns, independently of its source; Single scopes end on zone change.
    Indefinite,
    /// Expires at the next cleanup step (CR 514.2). One-shot effects created by a resolving
    /// spell or ability (Giant Growth, firebreathing) — independent of their source once made
    /// (CR 611.2g), so they persist even if the source permanent later leaves the battlefield.
    UntilEndOfTurn,
    /// CR 604.3 / 611.3: a continuous effect generated by a permanent's *static* ability (an
    /// anthem such as Glorious Anthem or Lord of Atlantis). It exists only while that permanent
    /// is on the battlefield, so the engine drains it when the source leaves (LTB), not at
    /// cleanup. The source is identified by [`ContinuousEffect::source_id`].
    WhileSourceOnBattlefield,
}

/// The kind of modification a continuous effect applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControllerReference {
    /// A controller fixed when a resolving spell or ability creates the effect.
    Fixed(i32),
    /// The current layer-2 controller of the continuous effect's battlefield source.
    SourceController,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ReturnController {
    #[default]
    Owner,
    AbilityController,
}

/// Which event-bound card a return trigger follows through its first zone change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggeredCardReference {
    AbilitySource,
    TriggerObject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContinuousEffectKind {
    /// CR 613.1b layer 2 — change control of the affected permanent.
    Layer2Control {
        controller: ControllerReference,
    },
    /// CR 612.8 / 613.1c layer 3 — replace every name the affected object has.
    Layer3SetName(String),
    /// CR 205.1b / 613.1d layer 4 — retain the existing type line and append these values.
    Layer4AddTypes(TypeLineAddition),
    /// CR 205.1a / 613.1d layer 4 — replace all card types and subtypes, preserving supertypes.
    Layer4SetTypeLine(TypeLineReplacement),
    /// CR 205.1b / 613.1d layer 4 — replace every creature type while retaining card types and
    /// unrelated subtypes. An empty list means the object loses all creature types. Frogify and
    /// Witness Protection exercise the nonempty form; Amoeboid Changeling exercises empty.
    Layer4SetCreatureTypes(Vec<String>),
    /// CR 613.1e layer 5 — replace every color of the affected object.
    Layer5SetColors(Vec<Color>),
    /// CR 613 layer 6 — remove every ability with timestamp precedence. Unable to Scream,
    /// Kenrith's Transformation, and Darksteel Mutation share this layer operation.
    Layer6RemoveAllAbilities,
    /// CR 613 layer 7b — set base power and toughness before modifiers and counters.
    Layer7bSetPt {
        power: i64,
        toughness: i64,
    },
    /// CR 101.2 / 116.2: prohibit a non-stack special action for affected permanents.
    ProhibitSpecialAction(SpecialActionKind),
    /// CR 613 layer 7c — modifying effects (+N/+N, -N/-N).
    PtModify {
        delta_power: i32,
        delta_toughness: i32,
    },
    /// CR 613 layer 7c dynamic self modifier from a safe pre-layer-7 battlefield count.
    PtModifyByCount {
        count: CountExpression,
        power_per_match: i32,
        toughness_per_match: i32,
    },
    /// CR 613 layer 6 — grant a keyword ability to affected permanents. Covers lords
    /// (Goblin Chieftain → Haste), pump sorceries (Overrun → Trample), and any
    /// "creatures you control gain [keyword] until end of turn" effect.
    Layer6AddKeyword(Keyword),
    /// CR 613.1f / 702.16: grant one parameterized protection ability.
    Layer6AddProtection(ProtectionQuality),
    /// CR 113.10 / 613.1f: grant an ordinary activated ability to the affected permanent.
    GrantActivatedAbility(Box<ActivatedAbilityDef>),
    /// CR 613.1f: grant an ordinary triggered ability to the affected permanent.
    GrantTriggeredAbility(Box<TriggeredAbilityDef>),
    CombatRestriction(CombatRestriction),
    /// CR 502.3: the affected permanent is excluded from its controller's normal untap-step
    /// turn-based action. This does not prohibit other spells or abilities from untapping it.
    DoesntUntapDuringUntapStep,
    /// CR 502.3: the affected permanent is excluded from its controller's normal untap-step
    /// action unless the source-relative live condition holds.
    DoesntUntapDuringUntapStepUnless(Box<GameCondition>),
    /// Absolute prohibition shared by Blossombind and Frozen in Ice.
    ProhibitUntap,
    /// CR 613.11 / 702.3: ignore only Defender while checking whether this creature may attack.
    /// The creature retains Defender for every other rules and display query.
    AttackAsThoughWithoutDefender,
    /// CR 305.2b / layer 5 (rule-change): controller may play `count` additional lands per turn.
    /// Covers Exploration, Oracle of Mul Daya, and similar enchantments/permanents.
    ExtraLandPlays(u32),
    /// CR 305.1 / 611.3: the affected player may play lands from their own graveyard while the
    /// source remains on the battlefield. Covers Icetill Explorer and Crucible of Worlds.
    PlayLandsFromOwnGraveyard,
}

#[cfg(test)]
mod issue_187_base_pt_tests {
    use super::*;

    #[test]
    fn source_relative_base_pt_requires_an_ability_source() {
        let effect = SpellEffectKind::SetBasePowerToughness {
            target: TargetFilter {
                kind: TargetKind::Creature,
                ..Default::default()
            },
            power: BasePowerToughnessValue::Source(PowerToughnessCharacteristic::Power),
            toughness: BasePowerToughnessValue::Source(PowerToughnessCharacteristic::Toughness),
        };
        assert!(effect.validate(EffectContext::Ability).is_ok());
        assert_eq!(
            effect.validate(EffectContext::Spell),
            Err("source-relative base P/T values require a permanent ability source".into())
        );
    }
}

#[cfg(test)]
mod issue_158_predicate_tests {
    use super::*;

    #[test]
    fn richer_public_predicates_validate_composable_filters() {
        let union = GameCondition::BattlefieldAggregate {
            filter: BattlefieldPermanentFilter {
                token: None,
                any_of: Some(vec![
                    BattlefieldPermanentFilter {
                        token: None,
                        any_of: None,
                        controllers: RelativePlayerSet::Controller,
                        card_type: Some(CardTypeFilter::Land),
                        color: None,
                        name: None,
                        required_subtypes: vec![],
                        exclude_source: false,
                    },
                    BattlefieldPermanentFilter {
                        token: None,
                        any_of: None,
                        controllers: RelativePlayerSet::Controller,
                        card_type: None,
                        color: None,
                        name: None,
                        required_subtypes: vec!["Treefolk".into()],
                        exclude_source: false,
                    },
                ]),
                controllers: RelativePlayerSet::Controller,
                card_type: None,
                color: None,
                name: None,
                required_subtypes: vec![],
                exclude_source: false,
            },
            aggregate: BattlefieldAggregate::DistinctNames,
            min: Some(7),
            max: None,
        };
        assert!(union.validate().is_ok());

        let graveyard = GameCondition::GraveyardAggregate {
            owners: RelativePlayerSet::Controller,
            aggregate: GraveyardAggregate::CardCount,
            filter: Some(ZoneCardFilter {
                subtype: Some("Lesson".into()),
                ..Default::default()
            }),
            min: Some(1),
            max: None,
        };
        assert!(graveyard.validate().is_ok());

        let tapped = GameCondition::BattlefieldCreatureCount {
            filter: BattlefieldCreatureCountFilter {
                controllers: RelativePlayerSet::Controller,
                tapped: Some(true),
                ..Default::default()
            },
            min: Some(2),
            max: None,
        };
        assert!(tapped.validate().is_ok());
    }

    #[test]
    fn committed_turn_predicates_validate_typed_bounds_and_identity() {
        let conditions = [
            GameCondition::CardsDrawnThisTurn {
                players: RelativePlayerSet::Controller,
                min: Some(2),
                max: None,
            },
            GameCondition::AttackersDeclaredThisTurn {
                players: RelativePlayerSet::Controller,
                filter: CreatureEventFilter {
                    required_subtypes: vec!["Spacecraft".into()],
                    ..Default::default()
                },
                min: Some(1),
                max: None,
            },
            GameCondition::PermanentsEnteredThisTurn {
                controllers: RelativePlayerSet::Controller,
                filter: PermanentEventFilter {
                    permanent_type: Some(PermanentTypeFilter::Artifact),
                    required_subtypes: vec![],
                    exclude_source: false,
                    ..Default::default()
                },
                min: Some(1),
                max: None,
            },
            GameCondition::SourceCounterCount {
                counter: CounterKind::MinusOneMinusOne,
                min: Some(1),
                max: None,
            },
            GameCondition::ObjectWasDealtDamageThisTurn {
                object: ConditionObjectRef::ChosenTarget {
                    group_index: 0,
                    target_index: 0,
                },
            },
        ];
        for condition in conditions {
            condition.validate().expect("valid issue #158 condition");
        }
    }
}
