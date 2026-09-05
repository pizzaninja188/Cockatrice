//! Public game-state conditions and their reusable filters.

use super::*;
use serde::{Deserialize, Serialize};

/// A public game-state predicate evaluated by the rules engine at the timing required by its
/// consumer: activation, trigger creation/resolution, or ordinary effect resolution.
///
/// The bounded count shape supports both boolean "a creature died" cards (Life Goes On,
/// Brimstone Volley) and count-sensitive consumers (Bloodcrazed Paladin, Lagomos) without exposing
/// the identities of the cards that moved through a graveyard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameCondition {
    /// Boolean conjunction across otherwise independent public game-state predicates. Every
    /// branch is evaluated against the same consumer context; Kaito, Bane of Nightmares and
    /// Gideon Blackblade combine turn ownership and loyalty checks through this reusable shape.
    AllOf(Vec<GameCondition>),
    /// Boolean disjunction across otherwise independent public game-state predicates. Every
    /// branch is evaluated against the same consumer context; this is the shared condition shape
    /// for Hidden Lair and the rest of its basic-land-enabled cycle.
    AnyOf(Vec<GameCondition>),
    /// CR 702.195: whether any selected player has the enduring-story designation.
    HasEnduringStory { players: RelativePlayerSet },
    /// Plasma Bolt and Temporal Intervention: a nonland permanent left the battlefield or
    /// a spell was cast for its Warp cost this turn, by any player.
    Void,
    /// Disappear (Insectoid Exterminator, Putrid Pals, and West Wind Avatar): whether any
    /// selected player controlled a permanent immediately before it left the battlefield this
    /// turn. Lands and tokens count; controller is captured from the committed departure event.
    PermanentLeftBattlefieldThisTurn { controllers: RelativePlayerSet },
    /// A face-authored condition captured after successful casting. Spell copies were not cast
    /// and have no result; ordinary live conditions elsewhere are unaffected.
    CastSnapshot { index: u32 },
    /// The zone from which the current physical spell was actually cast. This is captured as a
    /// face cast condition and consumed through `CastSnapshot`; spell copies were not cast.
    CastOrigin { origin: SpellCastOrigin },
    /// Compare the actual mana paid for the spell whose cast event created this triggered
    /// ability. The value is frozen at CR 601.2i and follows the trigger through resolution;
    /// printed mana value and aggregate Expend history are deliberately unrelated.
    TriggeringSpellManaSpent {
        comparison: SpellManaSpentComparison,
    },
    /// Whether the active player belongs to a player set relative to the condition's controller.
    /// `Controller` is "during your turn" (Daggersail Aeronaut); `Opponents` supports the inverse
    /// without assuming a two-player game.
    ActivePlayer { players: RelativePlayerSet },
    /// Star Charter checks either change to its controller; Flamecache Gecko checks loss by
    /// any opponent. Totals are separate, so offsetting changes still qualify (CR 119).
    LifeChangedThisTurn {
        players: ConditionPlayerSet,
        change: LifeChangeKind,
        quantifier: PlayerQuantifier,
    },
    /// Compare the minimum or maximum life total among a multiplayer-safe relative player set
    /// against signed inclusive bounds. Signed values preserve rules-correct comparisons after a
    /// player reaches zero or negative life but remains in the game until state-based actions.
    PlayerLifeAggregate {
        players: RelativePlayerSet,
        aggregate: PlayerLifeAggregate,
        #[serde(default)]
        min: Option<i32>,
        #[serde(default)]
        max: Option<i32>,
    },
    /// Descend (Deep Goblin Skulltaker, Enterprising Scallywag): destination-zone permanent
    /// cards, not battlefield creatures that died or cards still present in the graveyard.
    PermanentCardsEnteredGraveyardThisTurn {
        players: RelativePlayerSet,
        #[serde(default)]
        permanent_type: Option<PermanentTypeFilter>,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    /// Suspicious Detonation and Goblin Blast-Runner share committed sacrifice history.
    /// Tokens count; types belong to the permanent immediately before the sacrifice.
    PermanentsSacrificedThisTurn {
        players: RelativePlayerSet,
        #[serde(default)]
        permanent_type: Option<PermanentTypeFilter>,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    CreatureDeathsThisTurn {
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    /// Compare the number of spells cast this turn by the selected players. The count is
    /// committed only after CR 601.2 is complete, so rejected casts and spell copies do not
    /// contribute. Focus the Mind and flurry cards share this per-player history.
    SpellsCastThisTurn {
        players: RelativePlayerSet,
        #[serde(default)]
        filter: SpellCastFilter,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    /// Compare spells successfully cast during the immediately preceding turn. The history is
    /// retained as event facts so player-set and printed-characteristic filters remain reusable.
    SpellsCastLastTurn {
        players: RelativePlayerSet,
        #[serde(default)]
        filter: SpellCastFilter,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    /// Committed crimes, retained even when the spell or ability leaves the stack. Servant of
    /// the Stinger and Take for a Ride share this history predicate (not a timing permission).
    CrimesCommittedThisTurn {
        players: RelativePlayerSet,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    /// Compare committed successful draws by the selected players this turn.
    CardsDrawnThisTurn {
        players: RelativePlayerSet,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    /// Whether any selected player committed a nonempty attacker declaration this turn.
    /// Raid cards and "if you attacked this turn" abilities share this fact.
    AttackedThisTurn { players: RelativePlayerSet },
    /// Count declared attackers whose event-time creature characteristics match `filter`.
    AttackersDeclaredThisTurn {
        players: RelativePlayerSet,
        #[serde(default)]
        filter: CreatureEventFilter,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    /// Count permanent-entry facts captured with event-time characteristics.
    PermanentsEnteredThisTurn {
        controllers: RelativePlayerSet,
        #[serde(default)]
        filter: PermanentEventFilter,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    /// Compare one counter kind on the exact source object generation.
    SourceCounterCount {
        counter: CounterKind,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    /// Whether the referenced object generation was actually dealt positive damage this turn.
    ObjectWasDealtDamageThisTurn { object: ConditionObjectRef },
    /// Compare the current tapped status of a bound object. Source references use
    /// generation-scoped last-known information when an intervening-if clause resolves after
    /// the source left the battlefield.
    ObjectTapped {
        object: ConditionObjectRef,
        tapped: bool,
    },
    /// Inspect the current derived characteristics of an exact object already bound by this
    /// stack item. Depressurize and Yip Yip! use this after an earlier instruction without
    /// adding the secondary predicate to cast-time target legality.
    ObjectMatches {
        object: ConditionObjectRef,
        filter: Box<TargetFilter>,
    },
    /// Compare the number of battlefield creatures matching derived characteristics against
    /// inclusive bounds. Winged Words uses `min: 1` plus Flying; subtype-based cost reductions
    /// and public activation/trigger conditions reuse the same filter.
    BattlefieldCreatureCount {
        filter: BattlefieldCreatureCountFilter,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    /// Compare an aggregate of public battlefield permanents against inclusive bounds. This is
    /// the shared condition vocabulary for Scholar of Stars (count artifacts), Faerie Miscreant
    /// (count another permanent by effective name), and the power checks on Ornery Dilophosaur
    /// and Turret Ogre.
    BattlefieldAggregate {
        filter: BattlefieldPermanentFilter,
        aggregate: BattlefieldAggregate,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    /// Count public unlocked door designations among Rooms controlled by the selected players.
    /// This is a door count, not a Room count: a fully unlocked Room contributes two.
    UnlockedRoomDoorCount {
        controllers: RelativePlayerSet,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    /// Compare an aggregate of nontoken cards in the selected players' public graveyards against
    /// inclusive bounds. Threshold uses `CardCount`; delirium uses `DistinctCardTypes`.
    GraveyardAggregate {
        owners: RelativePlayerSet,
        aggregate: GraveyardAggregate,
        /// Optional printed-characteristic filter for the public graveyard cohort.
        #[serde(default)]
        filter: Option<ZoneCardFilter>,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
}

impl GameCondition {
    pub(crate) fn references_previous_effect_object(&self) -> bool {
        self.any_node_matches(|condition| {
            matches!(
                condition,
                Self::ObjectMatches {
                    object: ConditionObjectRef::PreviousEffectObject,
                    ..
                }
            )
        })
    }

    pub(crate) fn requires_triggering_spell_context(&self) -> bool {
        match self {
            Self::TriggeringSpellManaSpent { .. } => true,
            Self::AllOf(branches) | Self::AnyOf(branches) => {
                branches.iter().any(Self::requires_triggering_spell_context)
            }
            _ => false,
        }
    }

    /// Validate a condition in a context without a completed spell cast (costs, abilities,
    /// continuous effects, and the snapshot declarations themselves).
    pub(crate) fn validate_live(&self) -> Result<(), String> {
        if self.references_previous_effect_object() {
            return Err(
                "PreviousEffectObject is available only to an inline Conditional effect".into(),
            );
        }
        if self.requires_triggering_spell_context() {
            return Err("triggering-spell mana spending requires a spell-cast trigger".into());
        }
        if self.any_node_matches(|condition| matches!(condition, Self::CastOrigin { .. })) {
            return Err("CastOrigin is available only as a face cast condition".into());
        }
        self.validate_cast_snapshot_reference(0)?;
        self.validate()
    }

    pub(crate) fn validate_cast_condition(&self) -> Result<(), String> {
        if self.references_previous_effect_object() {
            return Err("PreviousEffectObject is unavailable to face cast conditions".into());
        }
        if self.requires_triggering_spell_context() {
            return Err("triggering-spell mana spending requires a spell-cast trigger".into());
        }
        self.validate_cast_snapshot_reference(0)?;
        self.validate()
    }

    pub(crate) fn validate_trigger_condition(&self) -> Result<(), String> {
        if self.references_previous_effect_object() {
            return Err(
                "PreviousEffectObject is unavailable to trigger and intervening-if conditions"
                    .into(),
            );
        }
        self.validate_cast_snapshot_reference(0)?;
        self.validate()
    }

    /// Return whether this condition or any nested disjunct satisfies a structural predicate.
    /// Registry consumers use this to keep context-specific authoring restrictions recursive.
    pub(crate) fn any_node_matches(
        &self,
        predicate: impl Copy + Fn(&GameCondition) -> bool,
    ) -> bool {
        predicate(self)
            || match self {
                Self::AllOf(branches) | Self::AnyOf(branches) => branches
                    .iter()
                    .any(|branch| branch.any_node_matches(predicate)),
                _ => false,
            }
    }

    pub(crate) fn validate_cast_snapshot_reference(&self, count: usize) -> Result<(), String> {
        match self {
            Self::CastSnapshot { index } if *index as usize >= count => {
                return Err(
                    "CastSnapshot requires an existing condition on the resolving spell face"
                        .into(),
                );
            }
            Self::AllOf(branches) | Self::AnyOf(branches) => {
                for branch in branches {
                    branch.validate_cast_snapshot_reference(count)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            GameCondition::AllOf(branches) => {
                if branches.len() < 2 {
                    return Err("condition AllOf requires at least two branches".into());
                }
                for (index, branch) in branches.iter().enumerate() {
                    branch.validate()?;
                    if branches[..index].contains(branch) {
                        return Err("condition AllOf cannot contain duplicate branches".into());
                    }
                }
                Ok(())
            }
            GameCondition::AnyOf(branches) => {
                if branches.len() < 2 {
                    return Err("condition AnyOf requires at least two branches".into());
                }
                for (index, branch) in branches.iter().enumerate() {
                    branch.validate()?;
                    if branches[..index].contains(branch) {
                        return Err("condition AnyOf cannot contain duplicate branches".into());
                    }
                }
                Ok(())
            }
            GameCondition::HasEnduringStory { .. }
            | GameCondition::Void
            | GameCondition::PermanentLeftBattlefieldThisTurn { .. }
            | GameCondition::CastSnapshot { .. }
            | GameCondition::CastOrigin { .. }
            | GameCondition::TriggeringSpellManaSpent { .. }
            | GameCondition::ObjectTapped { .. } => Ok(()),
            GameCondition::ActivePlayer { .. } => Ok(()),
            GameCondition::LifeChangedThisTurn { .. } => Ok(()),
            GameCondition::PlayerLifeAggregate { min, max, .. } => {
                validate_optional_bounds(min.as_ref(), max.as_ref(), "PlayerLifeAggregate")
            }
            GameCondition::CreatureDeathsThisTurn { min, max } => {
                validate_optional_bounds(min.as_ref(), max.as_ref(), "CreatureDeathsThisTurn")
            }
            GameCondition::SpellsCastThisTurn { min, max, .. }
            | GameCondition::SpellsCastLastTurn { min, max, .. }
            | GameCondition::PermanentCardsEnteredGraveyardThisTurn { min, max, .. }
            | GameCondition::PermanentsSacrificedThisTurn { min, max, .. }
            | GameCondition::CrimesCommittedThisTurn { min, max, .. }
            | GameCondition::CardsDrawnThisTurn { min, max, .. }
            | GameCondition::AttackersDeclaredThisTurn { min, max, .. }
            | GameCondition::PermanentsEnteredThisTurn { min, max, .. }
            | GameCondition::SourceCounterCount { min, max, .. } => {
                if let GameCondition::AttackersDeclaredThisTurn { filter, .. } = self {
                    filter.validate()?;
                }
                if let GameCondition::PermanentsEnteredThisTurn { filter, .. } = self {
                    filter.validate()?;
                }
                if let GameCondition::SpellsCastThisTurn { filter, .. } = self {
                    filter.validate()?;
                }
                if let GameCondition::SpellsCastLastTurn { filter, .. } = self {
                    filter.validate()?;
                }
                if let GameCondition::SourceCounterCount { counter, .. } = self {
                    counter.validate()?;
                }
                validate_optional_bounds(min.as_ref(), max.as_ref(), "bounded game condition")
            }
            GameCondition::AttackedThisTurn { .. } => Ok(()),
            GameCondition::ObjectWasDealtDamageThisTurn { .. } => Ok(()),
            GameCondition::ObjectMatches { filter, .. } => {
                filter.validate_characteristic_constraints()
            }
            GameCondition::BattlefieldCreatureCount { filter, min, max } => {
                filter.validate()?;
                validate_optional_bounds(min.as_ref(), max.as_ref(), "BattlefieldCreatureCount")
            }
            GameCondition::BattlefieldAggregate {
                filter, min, max, ..
            } => {
                filter.validate()?;
                validate_optional_bounds(min.as_ref(), max.as_ref(), "BattlefieldAggregate")
            }
            GameCondition::UnlockedRoomDoorCount { min, max, .. } => {
                validate_optional_bounds(min.as_ref(), max.as_ref(), "UnlockedRoomDoorCount")
            }
            GameCondition::GraveyardAggregate { min, max, .. } => {
                validate_optional_bounds(min.as_ref(), max.as_ref(), "GraveyardAggregate")
            }
        }
    }

    pub fn matches_value(&self, value: u32) -> bool {
        match self {
            GameCondition::AllOf(_)
            | GameCondition::AnyOf(_)
            | GameCondition::HasEnduringStory { .. }
            | GameCondition::Void
            | GameCondition::PermanentLeftBattlefieldThisTurn { .. }
            | GameCondition::CastSnapshot { .. }
            | GameCondition::CastOrigin { .. }
            | GameCondition::TriggeringSpellManaSpent { .. }
            | GameCondition::ActivePlayer { .. }
            | GameCondition::LifeChangedThisTurn { .. }
            | GameCondition::PlayerLifeAggregate { .. }
            | GameCondition::AttackedThisTurn { .. }
            | GameCondition::ObjectWasDealtDamageThisTurn { .. }
            | GameCondition::ObjectTapped { .. }
            | GameCondition::ObjectMatches { .. } => false,
            GameCondition::CreatureDeathsThisTurn { min, max }
            | GameCondition::PermanentCardsEnteredGraveyardThisTurn { min, max, .. }
            | GameCondition::PermanentsSacrificedThisTurn { min, max, .. }
            | GameCondition::SpellsCastThisTurn { min, max, .. }
            | GameCondition::SpellsCastLastTurn { min, max, .. }
            | GameCondition::CrimesCommittedThisTurn { min, max, .. }
            | GameCondition::CardsDrawnThisTurn { min, max, .. }
            | GameCondition::AttackersDeclaredThisTurn { min, max, .. }
            | GameCondition::PermanentsEnteredThisTurn { min, max, .. }
            | GameCondition::SourceCounterCount { min, max, .. }
            | GameCondition::BattlefieldCreatureCount { min, max, .. }
            | GameCondition::BattlefieldAggregate { min, max, .. }
            | GameCondition::UnlockedRoomDoorCount { min, max, .. }
            | GameCondition::GraveyardAggregate { min, max, .. } => {
                matches_optional_bounds(value, *min, *max)
            }
        }
    }

    pub fn matches_life_value(&self, value: i32) -> bool {
        match self {
            GameCondition::PlayerLifeAggregate { min, max, .. } => {
                matches_optional_bounds(value, *min, *max)
            }
            _ => false,
        }
    }
}

fn validate_optional_bounds<T: PartialOrd>(
    min: Option<&T>,
    max: Option<&T>,
    label: &str,
) -> Result<(), String> {
    if min.is_none() && max.is_none() {
        return Err(format!("{label} requires at least one of min or max"));
    }
    if min
        .zip(max)
        .is_some_and(|(minimum, maximum)| minimum > maximum)
    {
        return Err(format!("{label} min cannot exceed max"));
    }
    Ok(())
}

fn matches_optional_bounds<T: PartialOrd>(value: T, min: Option<T>, max: Option<T>) -> bool {
    min.is_none_or(|minimum| value >= minimum) && max.is_none_or(|maximum| value <= maximum)
}

/// Event-scoped comparisons shared by fixed-threshold Opus abilities and source-relative
/// Increment abilities. These are intentionally narrower than a general expression language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellManaSpentComparison {
    AtLeast(u64),
    GreaterThanSourcePowerOrToughness,
}

/// How selected players' public life totals collapse to one condition value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerLifeAggregate {
    Minimum,
    Maximum,
}

/// Gain/loss predicates for Star Charter and Flamecache Gecko; Either is a disjunction,
/// never a net-total comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifeChangeKind {
    Gain,
    Loss,
    Either,
}

/// Relative conditions (Star Charter/Flamecache Gecko) or a stack-bound selected player
/// (Thought-Stalker Warlock). This selects an existing target; it does not create targeting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionPlayerSet {
    Relative(RelativePlayerSet),
    /// Event-time player, for cast counts (Magebane Lizard) and life-history conditions.
    /// Without trigger context this selects nobody, never the controller as a fallback.
    AffectedPlayer,
    ChosenTarget {
        group_index: u32,
        target_index: u32,
    },
}

/// Distinguishes existential and universal player conditions without summing players' facts.
/// Both require a nonempty set. Applies to controller and opponent life-history mechanics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerQuantifier {
    Any,
    All,
}

/// Public characteristics used to select battlefield permanents for a [`GameCondition`].
/// All fields compose with logical AND. Controller, type, and power are derived values; `name`
/// uses the effective copiable face name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattlefieldPermanentFilter {
    /// Druid of the Spade and token-based sacrifice/activation cohorts use real token identity.
    #[serde(default)]
    pub token: Option<bool>,
    /// Optional recursive disjunction. The leaf predicates on this node still apply to every
    /// branch; a permanent that matches more than one branch is returned only once.
    #[serde(default)]
    pub any_of: Option<Vec<Self>>,
    pub controllers: RelativePlayerSet,
    #[serde(default)]
    pub card_type: Option<CardTypeFilter>,
    /// Derived color after copy and color-changing effects. Gearsmith Guardian uses `Blue`.
    #[serde(default)]
    pub color: Option<Color>,
    #[serde(default)]
    pub name: Option<String>,
    /// Every listed derived subtype must be present.
    #[serde(default)]
    pub required_subtypes: Vec<String>,
    /// "Another" excludes only the source object generation that created the condition.
    #[serde(default)]
    pub exclude_source: bool,
}

impl BattlefieldPermanentFilter {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if let Some(branches) = &self.any_of {
            if branches.len() < 2 {
                return Err(
                    "battlefield permanent filter any_of requires at least two branches".into(),
                );
            }
            for branch in branches {
                branch.validate()?;
            }
        }
        if self
            .name
            .as_ref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return Err("battlefield permanent filter name cannot be empty".into());
        }
        if self
            .required_subtypes
            .iter()
            .any(|subtype| subtype.trim().is_empty())
        {
            return Err("battlefield permanent filter subtype cannot be empty".into());
        }
        Ok(())
    }
}

/// Which public number a battlefield condition observes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BattlefieldAggregate {
    #[default]
    Count,
    DistinctNames,
    TotalPower,
    MaximumPower,
}

/// Which public number a graveyard condition observes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraveyardAggregate {
    CardCount,
    DistinctCardTypes,
}

/// Event-time permanent characteristics retained in turn history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PermanentEventFilter {
    #[serde(default)]
    pub permanent_type: Option<PermanentTypeFilter>,
    #[serde(default)]
    pub required_subtypes: Vec<String>,
    /// Exclude the exact source object generation ("another").
    #[serde(default)]
    pub exclude_source: bool,
    /// Carrot Cake's self-sacrifice and source branches such as Slagstone Refinery.
    #[serde(default)]
    pub source_only: bool,
    /// Celebration excludes lands even when they have another permanent type.
    #[serde(default)]
    pub excluded_types: Vec<PermanentTypeFilter>,
    /// Any one subtype suffices (Rakish Crew and Vial Smasher's outlaws).
    #[serde(default)]
    pub any_subtypes: Vec<String>,
    /// Knightfisher and Slagstone Refinery distinguish cards from tokens.
    #[serde(default)]
    pub token: Option<bool>,
    #[serde(default)]
    pub owner: Option<super::CastTriggerPlayer>,
    /// AND with the outer constraints; OR across branches, matching an object only once.
    #[serde(default)]
    pub any_of: Option<Vec<PermanentEventFilter>>,
}

impl PermanentEventFilter {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self
            .required_subtypes
            .iter()
            .chain(&self.any_subtypes)
            .any(|subtype| subtype.trim().is_empty())
        {
            return Err("permanent event subtype cannot be empty".into());
        }
        if self.source_only && self.exclude_source {
            return Err("permanent event cannot both require and exclude its source".into());
        }
        if self
            .permanent_type
            .is_some_and(|kind| self.excluded_types.contains(&kind))
        {
            return Err("permanent event cannot both require and exclude a type".into());
        }
        if let Some(branches) = &self.any_of {
            if branches.is_empty() {
                return Err("permanent event alternatives cannot be empty".into());
            }
            for branch in branches {
                branch.validate()?;
            }
        }
        Ok(())
    }
}

/// Stable object reference available while evaluating a stack-bound condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionObjectRef {
    Source,
    ChosenTarget {
        group_index: u32,
        target_index: u32,
    },
    /// The exact generation-bound object published by the immediately preceding resolution
    /// instruction. This is available only to an inline `Conditional` effect.
    PreviousEffectObject,
}

/// Which battlefield creatures contribute to a [`CountExpression`]. This is deliberately separate
/// from [`CreatureScopeFilter`]: count predicates may inspect derived keywords, while a keyword-dependent
/// continuous-effect scope would need CR 613 dependency ordering inside layer 6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattlefieldCreatureCountFilter {
    /// Whose controlled creatures are counted, relative to the resolving spell or ability.
    pub controllers: RelativePlayerSet,
    /// If present, the creature must have this derived subtype.
    #[serde(default)]
    pub subtype: Option<String>,
    /// Every listed keyword must be present in the creature's derived characteristics.
    #[serde(default)]
    pub required_keywords: Vec<Keyword>,
    /// If present, the creature must have the matching current tapped status.
    #[serde(default)]
    pub tapped: Option<bool>,
    /// If true, the creature must currently have at least one counter of any kind. This is a
    /// live physical-object predicate used by mechanics such as Delta Bloodflies, rather than a
    /// derived-characteristic inference from a specific counter vocabulary.
    #[serde(default)]
    pub requires_any_counter: bool,
    /// If present, the creature must currently have at least one counter of this kind.
    #[serde(default)]
    pub required_counter: Option<CounterKind>,
    /// Exclude the resolving spell or ability's physical source object.
    #[serde(default)]
    pub exclude_source: bool,
}

impl Default for BattlefieldCreatureCountFilter {
    fn default() -> Self {
        Self {
            controllers: RelativePlayerSet::Controller,
            subtype: None,
            required_keywords: Vec::new(),
            tapped: None,
            requires_any_counter: false,
            required_counter: None,
            exclude_source: false,
        }
    }
}

impl BattlefieldCreatureCountFilter {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self
            .subtype
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err("battlefield creature count subtype cannot be empty".into());
        }
        if self.requires_any_counter && self.required_counter.is_some() {
            return Err(
                "battlefield creature count filter cannot combine any-counter and specific-counter requirements"
                    .into(),
            );
        }
        Ok(())
    }
}
