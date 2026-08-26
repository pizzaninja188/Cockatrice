//! Spell and continuous-effect vocabulary plus shared effect parameters.

use super::{
    ActivatedAbilityDef, CardTypeFilter, CastCostReceiptCondition, Color, CreatureEventFilter,
    CreatureScopeFilter, GraveyardDestination, GraveyardFilter, Keyword, PermanentTypeFilter,
    PowerComparison, ProtectionQuality, ReflexiveTriggeredAbilityDef, SpecialActionKind,
    TargetController, TargetFilter, TargetKind, TargetRole, TriggeredAbilityDef, TypeLineAddition,
};
use crate::ManaCost;
use serde::de::{EnumAccess, MapAccess, SeqAccess, VariantAccess};
use serde::ser::SerializeStructVariant;
use serde::{Deserialize, Serialize};

fn default_one() -> u32 {
    1
}
use std::fmt;

/// A public game-state predicate evaluated by the rules engine at the timing required by its
/// consumer: activation, trigger creation/resolution, or ordinary effect resolution.
///
/// The bounded count shape supports both boolean "a creature died" cards (Life Goes On,
/// Brimstone Volley) and count-sensitive consumers (Bloodcrazed Paladin, Lagomos) without exposing
/// the identities of the cards that moved through a graveyard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameCondition {
    /// Whether the active player belongs to a player set relative to the condition's controller.
    /// `Controller` is "during your turn" (Daggersail Aeronaut); `Opponents` supports the inverse
    /// without assuming a two-player game.
    ActivePlayer { players: RelativePlayerSet },
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
    pub fn validate(&self) -> Result<(), String> {
        match self {
            GameCondition::ActivePlayer { .. } => Ok(()),
            GameCondition::PlayerLifeAggregate { min, max, .. } => {
                if min.is_none() && max.is_none() {
                    return Err("PlayerLifeAggregate requires at least one of min or max".into());
                }
                if min
                    .as_ref()
                    .zip(max.as_ref())
                    .is_some_and(|(minimum, maximum)| minimum > maximum)
                {
                    return Err("PlayerLifeAggregate min cannot exceed max".into());
                }
                Ok(())
            }
            GameCondition::CreatureDeathsThisTurn { min, max } => {
                if min.is_none() && max.is_none() {
                    return Err("CreatureDeathsThisTurn requires at least one of min or max".into());
                }
                if min
                    .as_ref()
                    .zip(max.as_ref())
                    .is_some_and(|(minimum, maximum)| minimum > maximum)
                {
                    return Err("CreatureDeathsThisTurn min cannot exceed max".into());
                }
                Ok(())
            }
            GameCondition::SpellsCastThisTurn { min, max, .. }
            | GameCondition::CardsDrawnThisTurn { min, max, .. }
            | GameCondition::AttackersDeclaredThisTurn { min, max, .. }
            | GameCondition::PermanentsEnteredThisTurn { min, max, .. }
            | GameCondition::SourceCounterCount { min, max, .. } => {
                if min.is_none() && max.is_none() {
                    return Err("bounded game condition requires at least one of min or max".into());
                }
                if min
                    .as_ref()
                    .zip(max.as_ref())
                    .is_some_and(|(minimum, maximum)| minimum > maximum)
                {
                    return Err("bounded game condition min cannot exceed max".into());
                }
                if let GameCondition::AttackersDeclaredThisTurn { filter, .. } = self {
                    filter.validate()?;
                }
                if let GameCondition::PermanentsEnteredThisTurn { filter, .. } = self {
                    filter.validate()?;
                }
                if let GameCondition::SourceCounterCount { counter, .. } = self {
                    counter.validate()?;
                }
                Ok(())
            }
            GameCondition::AttackedThisTurn { .. } => Ok(()),
            GameCondition::ObjectWasDealtDamageThisTurn { .. } => Ok(()),
            GameCondition::BattlefieldCreatureCount { filter, min, max } => {
                filter.validate()?;
                if min.is_none() && max.is_none() {
                    return Err(
                        "BattlefieldCreatureCount requires at least one of min or max".into(),
                    );
                }
                if min
                    .as_ref()
                    .zip(max.as_ref())
                    .is_some_and(|(minimum, maximum)| minimum > maximum)
                {
                    return Err("BattlefieldCreatureCount min cannot exceed max".into());
                }
                Ok(())
            }
            GameCondition::BattlefieldAggregate {
                filter, min, max, ..
            } => {
                filter.validate()?;
                if min.is_none() && max.is_none() {
                    return Err("BattlefieldAggregate requires at least one of min or max".into());
                }
                if min
                    .as_ref()
                    .zip(max.as_ref())
                    .is_some_and(|(minimum, maximum)| minimum > maximum)
                {
                    return Err("BattlefieldAggregate min cannot exceed max".into());
                }
                Ok(())
            }
            GameCondition::UnlockedRoomDoorCount { min, max, .. } => {
                if min.is_none() && max.is_none() {
                    return Err("UnlockedRoomDoorCount requires at least one of min or max".into());
                }
                if min
                    .as_ref()
                    .zip(max.as_ref())
                    .is_some_and(|(minimum, maximum)| minimum > maximum)
                {
                    return Err("UnlockedRoomDoorCount min cannot exceed max".into());
                }
                Ok(())
            }
            GameCondition::GraveyardAggregate { min, max, .. } => {
                if min.is_none() && max.is_none() {
                    return Err("GraveyardAggregate requires at least one of min or max".into());
                }
                if min
                    .as_ref()
                    .zip(max.as_ref())
                    .is_some_and(|(minimum, maximum)| minimum > maximum)
                {
                    return Err("GraveyardAggregate min cannot exceed max".into());
                }
                Ok(())
            }
        }
    }

    pub fn matches_value(&self, value: u32) -> bool {
        match self {
            GameCondition::ActivePlayer { .. }
            | GameCondition::PlayerLifeAggregate { .. }
            | GameCondition::AttackedThisTurn { .. }
            | GameCondition::ObjectWasDealtDamageThisTurn { .. } => false,
            GameCondition::CreatureDeathsThisTurn { min, max }
            | GameCondition::SpellsCastThisTurn { min, max, .. }
            | GameCondition::CardsDrawnThisTurn { min, max, .. }
            | GameCondition::AttackersDeclaredThisTurn { min, max, .. }
            | GameCondition::PermanentsEnteredThisTurn { min, max, .. }
            | GameCondition::SourceCounterCount { min, max, .. }
            | GameCondition::BattlefieldCreatureCount { min, max, .. }
            | GameCondition::BattlefieldAggregate { min, max, .. }
            | GameCondition::UnlockedRoomDoorCount { min, max, .. }
            | GameCondition::GraveyardAggregate { min, max, .. } => {
                min.is_none_or(|minimum| value >= minimum)
                    && max.is_none_or(|maximum| value <= maximum)
            }
        }
    }

    pub fn matches_life_value(&self, value: i32) -> bool {
        match self {
            GameCondition::PlayerLifeAggregate { min, max, .. } => {
                min.is_none_or(|minimum| value >= minimum)
                    && max.is_none_or(|maximum| value <= maximum)
            }
            _ => false,
        }
    }
}

/// How selected players' public life totals collapse to one condition value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerLifeAggregate {
    Minimum,
    Maximum,
}

/// Public characteristics used to select battlefield permanents for a [`GameCondition`].
/// All fields compose with logical AND. Controller, type, and power are derived values; `name`
/// uses the effective copiable face name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattlefieldPermanentFilter {
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
}

impl PermanentEventFilter {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self
            .required_subtypes
            .iter()
            .any(|subtype| subtype.trim().is_empty())
        {
            return Err("permanent event subtype cannot be empty".into());
        }
        Ok(())
    }
}

/// Stable object reference available while evaluating a stack-bound condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionObjectRef {
    Source,
    ChosenTarget { group_index: u32, target_index: u32 },
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

/// A public game-state count usable by any [`Amount`] consumer. Keeping the counted quantity
/// separate from the effect lets entry replacements, life gain, damage, P/T modifiers, and token
/// creation share one authoritative evaluator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CountExpression {
    /// Count battlefield creatures using their fully derived characteristics at evaluation time.
    BattlefieldCreatures {
        filter: BattlefieldCreatureCountFilter,
    },
    /// Count nontoken card objects with `name` in the selected players' public graveyards.
    /// `owners` is relative to the resolving spell or ability's controller.
    GraveyardCardsNamed {
        owners: RelativePlayerSet,
        name: String,
    },
    /// The committed, identity-free number of creatures that died during the current turn.
    /// Bloodcrazed Paladin shares this watcher with `GameCondition::CreatureDeathsThisTurn`.
    CreatureDeathsThisTurn,
    /// Count cards in an engine-owned payment or immediately preceding effect cohort. Gerrard's
    /// Verdict and Gorging Vulture share this without re-examining an object's current zone.
    CardsMatchingResult { filter: CardResultFilter },
}

impl CountExpression {
    fn validate(&self) -> Result<(), String> {
        match self {
            CountExpression::BattlefieldCreatures { filter } => filter.validate(),
            CountExpression::GraveyardCardsNamed { name, .. } if name.trim().is_empty() => {
                Err("graveyard card count name cannot be empty".into())
            }
            CountExpression::GraveyardCardsNamed { .. }
            | CountExpression::CreatureDeathsThisTurn
            | CountExpression::CardsMatchingResult { .. } => Ok(()),
        }
    }
}

/// An effect amount that is a fixed literal, the spell's cast-time X (CR 107.3), or a value
/// selected from public game state when the effect resolves.
///
/// In RON a bare integer (`amount: 3`) is [`Amount::Fixed`]; the string `amount: "X"` is the
/// chosen X, resolved from the resolving stack item's `chosen_x`. Custom (de)serialize keeps the
/// existing integer corpus untouched and roundtrips X as the string `"X"` (RON renders a bare
/// `X` identifier as an ambiguous unit value, so the quoted form is used). Applied to the
/// amount-bearing effects that can legally scale with X — the "name two cards" pair is Fireball
/// (`DamageTarget { amount: "X" }`) and Blue Sun's Zenith (`Draw { count: "X" }`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Amount {
    /// A literal count baked into the card data.
    Fixed(u32),
    /// The spell's cast-time X value (CR 107.3); resolved at resolution from `chosen_x`.
    X,
    /// Choose a literal from engine-owned public state as this effect resolves. Life Goes On and
    /// Brimstone Volley are the first two cards covered by this shared form.
    Conditional {
        condition: GameCondition,
        when_true: u32,
        otherwise: u32,
    },
    /// Count public game state using an engine-owned context. This is shared by entry
    /// replacements and ordinary resolving effects rather than being an entry-only mini-language.
    Count(CountExpression),
}

impl Amount {
    /// Resolve an amount that needs no game-state query. Dynamic amounts return `None` so a
    /// caller cannot accidentally choose a branch without consulting the authoritative engine.
    pub fn resolve_unconditional(&self, x: u32) -> Option<u32> {
        match self {
            Amount::Fixed(n) => Some(*n),
            Amount::X => Some(x),
            Amount::Conditional { .. } | Amount::Count(_) => None,
        }
    }

    /// True if this amount depends on the cast-time X.
    pub fn is_x(&self) -> bool {
        matches!(self, Amount::X)
    }

    pub fn requires_game_state(&self) -> bool {
        matches!(self, Amount::Conditional { .. } | Amount::Count(_))
    }

    pub(crate) fn card_result_filter(&self) -> Option<&CardResultFilter> {
        match self {
            Amount::Count(CountExpression::CardsMatchingResult { filter }) => Some(filter),
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Amount::Conditional { condition, .. } => condition.validate(),
            Amount::Count(expression) => expression.validate(),
            Amount::Fixed(_) | Amount::X => Ok(()),
        }
    }
}

impl From<u32> for Amount {
    fn from(n: u32) -> Self {
        Amount::Fixed(n)
    }
}

impl Serialize for Amount {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Amount::Fixed(n) => s.serialize_u32(*n),
            Amount::X => s.serialize_str("X"),
            Amount::Conditional {
                condition,
                when_true,
                otherwise,
            } => {
                let mut variant = s.serialize_struct_variant("Amount", 0, "Conditional", 3)?;
                variant.serialize_field("condition", condition)?;
                variant.serialize_field("when_true", when_true)?;
                variant.serialize_field("otherwise", otherwise)?;
                variant.end()
            }
            Amount::Count(expression) => {
                s.serialize_newtype_variant("Amount", 1, "Count", expression)
            }
        }
    }
}

#[derive(Deserialize)]
enum AmountVariant {
    Conditional,
    Count,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum ConditionalField {
    Condition,
    WhenTrue,
    Otherwise,
}

struct ConditionalAmountVisitor;

impl<'de> serde::de::Visitor<'de> for ConditionalAmountVisitor {
    type Value = Amount;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Conditional(condition: ..., when_true: ..., otherwise: ...)")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut condition = None;
        let mut when_true = None;
        let mut otherwise = None;
        while let Some(field) = map.next_key()? {
            match field {
                ConditionalField::Condition => {
                    if condition.is_some() {
                        return Err(serde::de::Error::duplicate_field("condition"));
                    }
                    condition = Some(map.next_value()?);
                }
                ConditionalField::WhenTrue => {
                    if when_true.is_some() {
                        return Err(serde::de::Error::duplicate_field("when_true"));
                    }
                    when_true = Some(map.next_value()?);
                }
                ConditionalField::Otherwise => {
                    if otherwise.is_some() {
                        return Err(serde::de::Error::duplicate_field("otherwise"));
                    }
                    otherwise = Some(map.next_value()?);
                }
            }
        }
        Ok(Amount::Conditional {
            condition: condition.ok_or_else(|| serde::de::Error::missing_field("condition"))?,
            when_true: when_true.ok_or_else(|| serde::de::Error::missing_field("when_true"))?,
            otherwise: otherwise.ok_or_else(|| serde::de::Error::missing_field("otherwise"))?,
        })
    }
}

impl<'de> Deserialize<'de> for Amount {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct AmountVisitor;
        impl<'de> serde::de::Visitor<'de> for AmountVisitor {
            type Value = Amount;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(
                    "a non-negative integer, the string \"X\", Conditional(...), or Count(...)",
                )
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Amount, E> {
                Ok(Amount::Fixed(v as u32))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Amount, E> {
                u32::try_from(v)
                    .map(Amount::Fixed)
                    .map_err(|_| E::custom("amount must be non-negative"))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Amount, E> {
                if v == "X" {
                    Ok(Amount::X)
                } else {
                    Err(E::custom(format!("unknown amount {v:?}, expected \"X\"")))
                }
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Amount, A::Error> {
                ConditionalAmountVisitor.visit_map(map)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Amount, A::Error> {
                let expression = seq
                    .next_element::<CountExpression>()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(serde::de::Error::invalid_length(2, &self));
                }
                Ok(Amount::Count(expression))
            }
            fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<Amount, A::Error> {
                let (variant, access) = data.variant::<AmountVariant>()?;
                match variant {
                    AmountVariant::Conditional => access.struct_variant(
                        &["condition", "when_true", "otherwise"],
                        ConditionalAmountVisitor,
                    ),
                    AmountVariant::Count => {
                        let expression: CountExpression = access.newtype_variant()?;
                        Ok(Amount::Count(expression))
                    }
                }
            }
        }
        d.deserialize_any(AmountVisitor)
    }
}

/// How a multi-target damage effect assigns its damage. This is shared by Fire (choose the
/// allocation while casting) and Fireball (divide evenly as it resolves), rather than baking a
/// card-specific branch into the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DamageDivision {
    #[default]
    ChooseAtCast,
    EvenAtResolution,
}

/// How much life a [`SpellEffectKind::LoseLife`] causes each recipient to lose.
///
/// Kept separate from [`Amount`] on purpose: its dynamic variants query shared engine context,
/// while `TargetManaValue` specifically needs the resolving spell's target, so folding it into
/// `Amount` would force target context through call sites where it is meaningless.
///
/// Named for widening — a later `TargetPower` (Rite of Consumption, Fling) or
/// `ManaValueOfRevealedCard` (Dark Confidant) is an added variant, not a rename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifeAmount {
    /// A literal amount baked into the card data (Thoughtseize: `Fixed(2)`).
    Fixed(u32),
    /// CR 202.3: the mana value of the object this spell targets (Reanimate). Legal only in an
    /// effect list that also contains a target-bearing effect — enforced at registry load.
    TargetManaValue,
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
    Chosen(TargetFilter),
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

/// An affine P/T bonus applied by [`SpellEffectKind::PumpTarget`]: resolve `amount` once, multiply
/// it by the signed per-unit deltas, then add those results to the effect's fixed P/T bonus.
/// Growth Cycle and Lavakin Brawler are the first spell and triggered-ability users.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PtScale {
    pub amount: Amount,
    pub power_per_unit: i32,
    pub toughness_per_unit: i32,
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
        Self::Chosen(TargetFilter::default_creature())
    }
}

/// Rule-level attack and block restrictions applied by static or resolving effects. Menace stays
/// a keyword because it constrains a completed blocking assignment rather than one creature or
/// attacker/blocker pair; both forms meet in the engine's shared block-legality pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CombatRestriction {
    #[serde(default)]
    pub cant_attack: bool,
    #[serde(default)]
    pub cant_block: bool,
    #[serde(default)]
    pub cant_be_blocked: bool,
}

impl CombatRestriction {
    pub fn is_empty(self) -> bool {
        !self.cant_attack && !self.cant_block && !self.cant_be_blocked
    }

    pub fn combine(&mut self, other: Self) {
        self.cant_attack |= other.cant_attack;
        self.cant_block |= other.cant_block;
        self.cant_be_blocked |= other.cant_be_blocked;
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
    Bottom,
    Shuffle,
    /// The targeted object's owner makes a logged resolution-time choice. Uncharted Voyage and
    /// Riverwalk Technique share this owner-relative placement primitive.
    OwnerChoiceTopOrBottom,
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
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolutionBranchDef {
    pub label: String,
    pub cost: ResolutionCost,
    #[serde(default)]
    pub requirement: ResolutionBranchRequirement,
    #[serde(default)]
    pub effects: Vec<SpellEffectKind>,
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
    Exile,
    Sacrifice,
    Mill,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ResolutionBranchSelection {
    /// Present every live branch to the deciding player through the existing choice contract.
    #[default]
    PlayerChoice,
    /// Resolve the first live branch in authored order without publishing a player choice.
    FirstApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CastCostConditionalAmount {
    pub condition: CastCostReceiptCondition,
    pub if_selected: u32,
    pub otherwise: u32,
}

/// Timing for a delayed trigger that sacrifices the full post-replacement token cohort.
/// Mobilize uses the next end step; Kav Landseeker uses the end step of its controller's next
/// actual turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelayedTokenSacrificeTiming {
    NextEndStep,
    ControllerNextTurnEndStep,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellEffectKind {
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
    /// the known-narrow one (it cannot express Earthquake). Note that `StackItem::chosen_x` is 0
    /// for triggered abilities, so `Amount::X` here resolves to 0 until an X-costed activated
    /// ability or spell uses this effect.
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
        count: u32,
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
    /// CR 701.5: counter target spell on the stack. `spell_filter` narrows which spells are legal
    /// targets — `None` is unrestricted (Counterspell), `Some(Creature)` is Essence Scatter,
    /// `Some(Noncreature)` is Negate. `unless_controller_pays` parks resolution for an optional
    /// generic-mana payment by that spell's controller (Convolute, Mana Leak). Reuses
    /// [`CardTypeFilter`] so any future "counter target X spell" needs no new variant.
    CounterTargetSpell {
        #[serde(default)]
        spell_filter: Option<CardTypeFilter>,
        #[serde(default)]
        unless_controller_pays: Option<u32>,
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
    /// the same way as [`Self::CounterTargetSpell`] — `Some(InstantOrSorcery)` for Twincast /
    /// Reverberate ("copy target instant or sorcery spell"); only spells (not abilities) qualify.
    CopyTargetSpell {
        #[serde(default = "one")]
        count: u32,
        #[serde(default)]
        spell_filter: Option<CardTypeFilter>,
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
    /// CR 613 layer 6: grant one or more keyword abilities to every creature matching `filter`
    /// until end of turn. Untargeted — the one-shot keyword-grant sibling of
    /// [`StaticAbilityDef::AnthemKeyword`]. Covers Overrun (Trample) and Make a Stand
    /// (Indestructible); attacking-creature keyword grants reuse the same snapshot filter.
    GrantKeywordsAll {
        #[serde(default)]
        filter: CreatureScopeFilter,
        keywords: Vec<Keyword>,
    },
    /// CR 613 layer 6: grant one or more keyword abilities until end of turn. `Chosen` is an
    /// ordinary permanent target (Boros Charm); `Source` auto-binds an activated or triggered
    /// ability to its own permanent without using the targeting path (Goblin Bird-Grabber).
    GrantKeywords {
        #[serde(default)]
        subject: EffectSubject,
        keywords: Vec<Keyword>,
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
    /// Return the card referenced by this death trigger, but only while it remains the immediate
    /// graveyard object produced by that death. Abnormal Endurance references the granted
    /// ability's source; Unholy Indenture references the enchanted creature observed by its Aura.
    ReturnTriggeredCardFromGraveyard {
        reference: TriggeredCardReference,
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
    ExileTarget,
    ExileTargetGainLifeEqualToPower,
    /// Exile the top card of the named player's library and let that player play the exact
    /// resulting object until the end of their next turn. Clockwork Percussionist and Impossible
    /// Inferno share this primitive; the engine owns physical identity, duration, and legality.
    ExileTopWithPlayPermission {
        player: PlayerRecipient,
    },
    /// Return a battlefield permanent to its owner's hand. A chosen subject uses the normal
    /// targeting contract (Unsummon, Boomerang); a source-bound subject is untargeted and keeps
    /// CR 400.7 generation identity (Wingspan Stride).
    ReturnToOwnersHand {
        subject: EffectSubject,
    },
    /// Move a chosen battlefield permanent to its owner's library (CR 400.3). The target filter
    /// carries card-specific restrictions; placement controls library ordering or shuffling.
    PutTargetPermanentInOwnersLibrary {
        target: TargetFilter,
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
    /// `Creature` covers Pyroclasm / Pestilence-style sweeps; `AnyPermanent` is reserved for
    /// future "damage to each permanent" effects. Only object kinds are legal (validated at load).
    DamageAll {
        amount: u32,
        #[serde(default = "TargetFilter::default_creature")]
        kind: TargetFilter,
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
    PutCounters {
        counter: CounterKind,
        count: u32,
        #[serde(default)]
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
    EffectSubject::Chosen(TargetFilter::default_creature())
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
}

/// CR 106.6 restriction carried by an individual mana contribution. Empty purpose lists mean
/// that purpose is disallowed. Filters within one list are ORed so one contribution can cover
/// wording such as "an Elemental spell or a Chandra planeswalker spell."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManaSpendingRestriction {
    /// Engine-authored public hover text; clients display it verbatim and never parse Oracle.
    pub label: String,
    #[serde(default)]
    pub cast_spell: Vec<ManaSpendFilter>,
    #[serde(default)]
    pub activate_ability: Vec<ManaSpendFilter>,
}

impl ManaSpendingRestriction {
    pub fn validate(&self) -> Result<(), String> {
        if self.label.trim().is_empty() {
            return Err("mana spending restriction label cannot be empty".into());
        }
        if self.cast_spell.is_empty() && self.activate_ability.is_empty() {
            return Err("mana spending restriction must allow a spell or ability purpose".into());
        }
        self.cast_spell
            .iter()
            .chain(&self.activate_ability)
            .try_for_each(ManaSpendFilter::validate)
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
            } | SpellEffectKind::GrantKeywords {
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
            } | SpellEffectKind::Destroy {
                subject: EffectSubject::AttachedObject,
            }
        )
    }

    pub(crate) fn uses_trigger_object_reference(&self) -> bool {
        matches!(
            self,
            SpellEffectKind::PumpTarget {
                subject: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::PutCounters {
                subject: EffectSubject::TriggerObject,
                ..
            } | SpellEffectKind::GrantKeywords {
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
            } | SpellEffectKind::ReturnTriggeredCardFromGraveyard {
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
            SpellEffectKind::CreatureDealsDamageEqualToPower { source, target } => {
                vec![TargetRole::Filtered(source), TargetRole::Filtered(target)]
            }
            SpellEffectKind::Fight { first, second } => [first, second]
                .into_iter()
                .filter_map(|subject| match subject {
                    EffectSubject::Chosen(filter) => Some(TargetRole::Filtered(filter)),
                    EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject => None,
                })
                .collect(),
            SpellEffectKind::Destroy { subject }
            | SpellEffectKind::PumpTarget { subject, .. }
            | SpellEffectKind::Tap { subject }
            | SpellEffectKind::Untap { subject }
            | SpellEffectKind::GrantKeywords { subject, .. }
            | SpellEffectKind::GrantProtection { subject, .. }
            | SpellEffectKind::GrantTriggeredAbility { subject, .. }
            | SpellEffectKind::CreateDelayedTrigger { subject, .. }
            | SpellEffectKind::AddTypes { subject, .. }
            | SpellEffectKind::ReturnToOwnersHand { subject }
            | SpellEffectKind::Regenerate { subject }
            | SpellEffectKind::PutCounters { subject, .. } => match subject {
                EffectSubject::Chosen(target) => vec![TargetRole::Filtered(target)],
                EffectSubject::Source
                | EffectSubject::AttachedObject
                | EffectSubject::TriggerObject => Vec::new(),
            },
            SpellEffectKind::ApplyCombatRestriction { scope, .. } => match scope {
                CombatRestrictionScope::Chosen(target) => vec![TargetRole::Filtered(target)],
                CombatRestrictionScope::Source | CombatRestrictionScope::Matching(_) => Vec::new(),
            },
            SpellEffectKind::DamageTarget { target, .. }
            | SpellEffectKind::ExileIfWouldDieThisTurn { target }
            | SpellEffectKind::DamageTargets { target, .. }
            | SpellEffectKind::DestroyAttached { target, .. }
            | SpellEffectKind::PutTargetPermanentInOwnersLibrary { target, .. }
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
            | SpellEffectKind::Equip { target }
            | SpellEffectKind::TargetPlayerSacrifices { target, .. }
            | SpellEffectKind::PreventNextDamage { target, .. }
            | SpellEffectKind::PreventAllCombatDamageToTargetTurn { target } => {
                vec![TargetRole::Filtered(target)]
            }
            SpellEffectKind::ExileTarget | SpellEffectKind::ExileTargetGainLifeEqualToPower => {
                vec![TargetRole::CreaturePermanent]
            }
            SpellEffectKind::CounterTargetSpell { spell_filter, .. }
            | SpellEffectKind::CopyTargetSpell { spell_filter, .. } => {
                vec![TargetRole::StackSpell(*spell_filter)]
            }
            SpellEffectKind::MoveGraveyardCards { filter, .. } => {
                vec![TargetRole::GraveyardCard(filter)]
            }
            SpellEffectKind::DamagePlayer { .. }
            | SpellEffectKind::DamageAttackedPlayerOrPlaneswalker { .. }
            | SpellEffectKind::Draw { .. }
            | SpellEffectKind::Discard { .. }
            | SpellEffectKind::DrawDiscard { .. }
            | SpellEffectKind::CounterTriggeringStackObjectUnlessPays { .. }
            | SpellEffectKind::ChooseResolutionBranch { .. }
            | SpellEffectKind::CreateReflexiveTrigger { .. }
            | SpellEffectKind::Scry { .. }
            | SpellEffectKind::LibraryPartition { .. }
            | SpellEffectKind::ManifestDread
            | SpellEffectKind::LookChooseToHand { .. }
            | SpellEffectKind::TapAllCreatures { .. }
            | SpellEffectKind::UntapAll { .. }
            | SpellEffectKind::PumpAll { .. }
            | SpellEffectKind::GrantKeywordsAll { .. }
            | SpellEffectKind::ReturnTriggeredCardFromGraveyard { .. }
            | SpellEffectKind::SacrificeObservedObjects
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
                CardResultAction::Exile => matches!(
                    effect,
                    SpellEffectKind::ExileCardsFromHand { .. }
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
                | SpellEffectKind::DamagePlayer { amount, .. }
                | SpellEffectKind::Draw { count: amount, .. }
                | SpellEffectKind::GainLife { amount }
                | SpellEffectKind::Mill { count: amount, .. }
                | SpellEffectKind::CreateTokens { count: amount, .. }
                | SpellEffectKind::CreateAttackingTokens { count: amount, .. } => Some(amount),
                SpellEffectKind::PumpTarget {
                    scale: Some(scale), ..
                } => Some(&scale.amount),
                _ => None,
            };
            let previous = index
                .checked_sub(1)
                .and_then(|previous| effects.get(previous));
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
        }
        Ok(())
    }

    /// Startup validation: reject effect/filter combinations the engine cannot honor.
    /// Returns `Err` with a human-readable reason; called from the card registry loader.
    /// `context` distinguishes spells from abilities so source-bound subjects are
    /// rejected where they make no sense.
    pub fn validate(&self, context: EffectContext) -> Result<(), String> {
        for filter in self.target_filters() {
            filter.validate_target_constraints()?;
        }
        if let SpellEffectKind::MoveGraveyardCards { filter, .. } = self {
            filter.validate()?;
        }
        if let SpellEffectKind::PutCounters { counter, .. } = self {
            counter.validate()?;
        }

        match self {
            SpellEffectKind::DamageTarget { amount, .. }
            | SpellEffectKind::DamagePlayer { amount, .. }
            | SpellEffectKind::Draw { count: amount, .. }
            | SpellEffectKind::GainLife { amount }
            | SpellEffectKind::Mill { count: amount, .. }
            | SpellEffectKind::CreateTokens { count: amount, .. }
            | SpellEffectKind::CreateAttackingTokens { count: amount, .. } => amount.validate()?,
            SpellEffectKind::PumpTarget {
                scale: Some(scale), ..
            } => scale.amount.validate()?,
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
            | SpellEffectKind::GrantKeywordsAll { filter, .. } => filter.validate()?,
            SpellEffectKind::ReturnTriggeredCardFromGraveyard { entry_counters, .. } => {
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
                for (branch_index, branch) in branches.iter().enumerate() {
                    let is_first_applicable_noop_fallback = *selection
                        == ResolutionBranchSelection::FirstApplicable
                        && branch_index + 1 == branches.len()
                        && matches!(branch.requirement, ResolutionBranchRequirement::Always)
                        && branch.cost == ResolutionCost::None;
                    if branch.label.trim().is_empty()
                        || (branch.effects.is_empty() && !is_first_applicable_noop_fallback)
                    {
                        return Err(
                            "resolution choice branches require a label and at least one effect"
                                .into(),
                        );
                    }
                    match &branch.cost {
                        ResolutionCost::None => {}
                        ResolutionCost::Mana(cost) => {
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
                        ResolutionCost::SacrificePermanent { filter } => {
                            filter.validate_target_constraints()?;
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
            } | SpellEffectKind::GrantKeywords {
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
            } | SpellEffectKind::Destroy {
                subject: EffectSubject::Source
                    | EffectSubject::AttachedObject
                    | EffectSubject::TriggerObject,
            } | SpellEffectKind::ChangeSourceFace { .. }
                | SpellEffectKind::ReturnTriggeredCardFromGraveyard { .. }
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
            | SpellEffectKind::GainControlUntilEndOfTurn { target }
            | SpellEffectKind::Tap {
                subject: EffectSubject::Chosen(target),
            }
            | SpellEffectKind::Untap {
                subject: EffectSubject::Chosen(target),
            }
            | SpellEffectKind::ReturnToOwnersHand {
                subject: EffectSubject::Chosen(target),
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
                if filter.any_terminal_filter_matches(|leaf| leaf.exclude_source) {
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
                if kind.any_terminal_filter_matches(|leaf| leaf.exclude_source) {
                    return Err("mass effect filter cannot exclude the effect source".into());
                }
                Ok(())
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
                } else if filter.any_terminal_filter_matches(|leaf| leaf.exclude_source) {
                    Err("GrantKeywordsAllPermanents filter cannot exclude the effect source".into())
                } else if keywords.is_empty() {
                    Err("GrantKeywordsAllPermanents requires at least one keyword".into())
                } else {
                    Ok(())
                }
            }
            SpellEffectKind::ApplyCombatRestriction { scope, restriction } => {
                if restriction.is_empty() {
                    return Err("ApplyCombatRestriction requires at least one restriction".into());
                }
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
                unless_controller_pays: Some(0),
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
                ResolutionCost::None | ResolutionCost::SacrificePermanent { .. } => {
                    Err("Ward supports only mana and discard-card costs".into())
                }
            },
            // Library searches use the resolution-interrupt machinery and are legal on spells
            // and nonmana abilities alike (Demonic Tutor, Evolving Wilds).
            SpellEffectKind::SearchLibrary {
                count,
                filter,
                zones,
                conditional_destination,
                count_by_cast_cost,
                ..
            } => {
                if *count == 0 {
                    return Err("SearchLibrary requires a positive count".into());
                }
                if count_by_cast_cost.is_some_and(|conditional| {
                    conditional.if_selected == 0 || conditional.otherwise == 0
                }) {
                    return Err("SearchLibrary cast-cost counts must be positive".into());
                }
                if let Some(filter) = filter {
                    filter.validate()?;
                }
                zones.validate()?;
                if let Some(conditional) = conditional_destination {
                    conditional.condition.validate()?;
                }
                Ok(())
            }
            SpellEffectKind::LookChooseToHand { count, filter, .. } => {
                if *count == 0 {
                    return Err("LookChooseToHand requires a positive count".into());
                }
                filter.validate()
            }
            SpellEffectKind::ChooseGraveyardCard { filter, .. } => filter.validate(),
            // CR 701.18: scry is legal on spells and on abilities alike (scry lands, Sensei's
            // Divining Top-style activations), so the only malformed case is scrying zero cards.
            SpellEffectKind::Scry { count } => {
                if *count == 0 {
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

/// Which event-bound card a graveyard-return trigger follows through its first zone change.
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
    /// CR 205.1b / 613.1d layer 4 — retain the existing type line and append these values.
    Layer4AddTypes(TypeLineAddition),
    /// CR 205.1b / 613.1d layer 4 — replace every creature type while retaining card types and
    /// unrelated subtypes. An empty list means the object loses all creature types. Frogify and
    /// Witness Protection exercise the nonempty form; Amoeboid Changeling exercises empty.
    Layer4SetCreatureTypes(Vec<String>),
    /// CR 613 layer 6 — remove every ability with timestamp precedence. Unable to Scream,
    /// Kenrith's Transformation, and Darksteel Mutation share this layer operation.
    Layer6RemoveAllAbilities,
    /// CR 613 layer 7b — set base power and toughness before modifiers and counters.
    Layer7bSetPt {
        power: u32,
        toughness: u32,
    },
    /// CR 101.2 / 116.2: prohibit a non-stack special action for affected permanents.
    ProhibitSpecialAction(SpecialActionKind),
    /// CR 613 layer 7c — modifying effects (+N/+N, -N/-N).
    PtModify {
        delta_power: i32,
        delta_toughness: i32,
    },
    /// CR 613 layer 7c dynamic self modifier from a battlefield-creature count.
    PtModifyByCreatureCount {
        filter: BattlefieldCreatureCountFilter,
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
    /// CR 613.11 / 702.3: ignore only Defender while checking whether this creature may attack.
    /// The creature retains Defender for every other rules and display query.
    AttackAsThoughWithoutDefender,
    /// CR 305.2b / layer 5 (rule-change): controller may play `count` additional lands per turn.
    /// Covers Exploration, Oracle of Mul Daya, and similar enchantments/permanents.
    ExtraLandPlays(u32),
    // Future: Layer7bSetPt { power: i32, toughness: i32 }, …
}

#[cfg(test)]
mod issue_158_predicate_tests {
    use super::*;

    #[test]
    fn richer_public_predicates_validate_composable_filters() {
        let union = GameCondition::BattlefieldAggregate {
            filter: BattlefieldPermanentFilter {
                any_of: Some(vec![
                    BattlefieldPermanentFilter {
                        any_of: None,
                        controllers: RelativePlayerSet::Controller,
                        card_type: Some(CardTypeFilter::Land),
                        color: None,
                        name: None,
                        required_subtypes: vec![],
                        exclude_source: false,
                    },
                    BattlefieldPermanentFilter {
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
