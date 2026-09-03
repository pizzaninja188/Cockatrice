//! Count, amount, and life-value expressions shared by effects and costs.

use super::*;
use serde::de::{EnumAccess, MapAccess, SeqAccess, VariantAccess};
use serde::ser::SerializeStructVariant;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A public game-state count usable by any [`Amount`] consumer. Keeping the counted quantity
/// separate from the effect lets entry replacements, life gain, damage, P/T modifiers, and token
/// creation share one authoritative evaluator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CountExpression {
    /// Magebane Lizard counts the caster's noncreature spells; Thunder Salvo excludes
    /// only the resolving spell's own committed occurrence, not an uncast copy's original.
    SpellsCastThisTurn {
        players: ConditionPlayerSet,
        #[serde(default)]
        filter: SpellCastFilter,
        #[serde(default)]
        exclude_source: bool,
    },
    /// Flow of Knowledge and Gold Rush count derived public permanent cohorts.
    BattlefieldPermanents { filter: BattlefieldPermanentFilter },
    /// Chupacabra Echo and Calamitous Cave-In count printed public-zone characteristics.
    GraveyardCards {
        owners: RelativePlayerSet,
        #[serde(default)]
        filter: Option<ZoneCardFilter>,
    },
    /// Repulsive Mutation and Glint Weaver use the greatest power/toughness respectively.
    BattlefieldMaximum {
        filter: BattlefieldPermanentFilter,
        characteristic: PowerToughnessCharacteristic,
    },
    /// Brambleguard Captain and Boulderbranch Golem use the original source's power.
    SourcePower,
    /// Witchstalker Frenzy and Search Party Captain count distinct declared creatures.
    DeclaredAttackers {
        players: RelativePlayerSet,
        #[serde(default)]
        filter: CreatureEventFilter,
    },
    /// A flat, signed affine sum (Calamitous Cave-In, Thunder Salvo). Nested sums are invalid.
    Affine {
        #[serde(default)]
        constant: i32,
        terms: Vec<QuantityTerm>,
    },
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
    /// Sum one derived P/T characteristic across distinct, generation-bound objects in an
    /// engine-owned result cohort. Station on Wurmwall Sweeper and Tapestry Warden reads the
    /// tapped creature's power on resolution; the typed characteristic keeps the evaluator
    /// reusable for corresponding toughness-based effects.
    CardResultCharacteristicSum {
        filter: CardResultFilter,
        characteristic: PowerToughnessCharacteristic,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerToughnessCharacteristic {
    Power,
    Toughness,
}

/// One independently authored value for a resolving base-power/toughness setter. Fixed setters
/// (Mind Transfer Protocol, Quandrix Charm) and source-relative setters (Galion) freeze their
/// signed values as the instruction resolves rather than remaining linked to the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BasePowerToughnessValue {
    Fixed(i64),
    Source(PowerToughnessCharacteristic),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantityTerm {
    pub coefficient: i32,
    pub quantity: CountExpression,
}

impl CountExpression {
    fn requires_source_power(&self) -> bool {
        match self {
            Self::SourcePower => true,
            Self::Affine { terms, .. } => terms
                .iter()
                .any(|term| term.quantity.requires_source_power()),
            _ => false,
        }
    }
    pub(crate) fn validate_static_count(&self) -> Result<(), String> {
        self.validate()?;
        match self {
            Self::BattlefieldPermanents { .. } => Ok(()),
            Self::BattlefieldCreatures { filter } if filter.required_keywords.is_empty() => Ok(()),
            _ => Err("static P/T scaling requires a pre-layer-7 battlefield count; other quantities require CR 613.8 dependency ordering".into()),
        }
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self {
            Self::SpellsCastThisTurn { filter, .. } => filter.validate(),
            Self::BattlefieldPermanents { filter } | Self::BattlefieldMaximum { filter, .. } => {
                filter.validate()
            }
            Self::GraveyardCards { filter, .. } => {
                filter.as_ref().map_or(Ok(()), ZoneCardFilter::validate)
            }
            Self::SourcePower => Ok(()),
            Self::DeclaredAttackers { filter, .. } => filter.validate(),
            Self::Affine { terms, .. } => {
                if terms.is_empty() {
                    return Err("affine quantity requires at least one term".into());
                }
                for term in terms {
                    if term.coefficient == 0
                        || matches!(
                            term.quantity,
                            Self::Affine { .. }
                                | Self::CardsMatchingResult { .. }
                                | Self::CardResultCharacteristicSum { .. }
                        )
                    {
                        return Err("affine quantity requires nonzero coefficients and public, non-affine leaves".into());
                    }
                    term.quantity.validate()?;
                }
                Ok(())
            }
            CountExpression::BattlefieldCreatures { filter } => filter.validate(),
            CountExpression::GraveyardCardsNamed { name, .. } if name.trim().is_empty() => {
                Err("graveyard card count name cannot be empty".into())
            }
            CountExpression::GraveyardCardsNamed { .. }
            | CountExpression::CreatureDeathsThisTurn
            | CountExpression::CardsMatchingResult { .. }
            | CountExpression::CardResultCharacteristicSum { .. } => Ok(()),
        }
    }

    fn card_result_filter(&self) -> Option<&CardResultFilter> {
        match self {
            Self::CardsMatchingResult { filter }
            | Self::CardResultCharacteristicSum { filter, .. } => Some(filter),
            Self::Affine { terms, .. } => terms
                .iter()
                .find_map(|term| term.quantity.card_result_filter()),
            _ => None,
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
    /// Cinder Strike / Burst Lightning: one instruction whose amount reads a committed cost.
    CastCost(CastCostConditionalAmount),
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
    pub(crate) fn requires_triggering_spell_context(&self) -> bool {
        matches!(
            self,
            Self::Conditional { condition, .. }
                if condition.requires_triggering_spell_context()
        )
    }

    pub(crate) fn validate_source_context(&self, has_source: bool) -> Result<(), String> {
        if !has_source
            && matches!(self, Self::Count(expression) if expression.requires_source_power())
        {
            return Err("source power requires an ability bound to a battlefield source".into());
        }
        Ok(())
    }

    pub(crate) fn validate_cost(&self, has_source: bool) -> Result<(), String> {
        self.validate_live()?;
        self.validate_source_context(has_source)?;
        if matches!(self, Self::X | Self::Fixed(0)) || self.card_result_filter().is_some() {
            return Err("generic reduction requires a nonzero literal or a public quantity available at cost determination".into());
        }
        Ok(())
    }

    pub(super) fn validate_effect(&self, context: EffectContext) -> Result<(), String> {
        self.validate()?;
        if context == EffectContext::Ability && matches!(self, Self::CastCost(_)) {
            return Err("cast-cost amount requires a spell's cast receipt".into());
        }
        if context == EffectContext::Spell && self.requires_triggering_spell_context() {
            return Err("spells cannot reference triggering-spell mana spending".into());
        }
        self.validate_source_context(context == EffectContext::Ability)
    }
    pub(crate) fn validate_cast_snapshot_references(&self, count: usize) -> Result<(), String> {
        if let Self::Conditional { condition, .. } = self {
            condition.validate_cast_snapshot_reference(count)?;
        }
        Ok(())
    }

    pub(crate) fn validate_live(&self) -> Result<(), String> {
        if matches!(self, Self::CastCost(_)) {
            return Err("cast-cost amount requires a resolving stack item".into());
        }
        self.validate_cast_snapshot_references(0)?;
        self.validate()
    }

    /// Resolve an amount that needs no game-state query. Dynamic amounts return `None` so a
    /// caller cannot accidentally choose a branch without consulting the authoritative engine.
    pub fn resolve_unconditional(&self, x: u32) -> Option<u32> {
        match self {
            Amount::Fixed(n) => Some(*n),
            Amount::X => Some(x),
            Amount::Conditional { .. } | Amount::Count(_) | Amount::CastCost(_) => None,
        }
    }

    /// True if this amount depends on the cast-time X.
    pub fn is_x(&self) -> bool {
        matches!(self, Amount::X)
    }

    pub fn requires_game_state(&self) -> bool {
        matches!(
            self,
            Amount::Conditional { .. } | Amount::Count(_) | Amount::CastCost(_)
        )
    }

    pub(crate) fn card_result_filter(&self) -> Option<&CardResultFilter> {
        match self {
            Amount::Count(expression) => expression.card_result_filter(),
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Amount::Conditional { condition, .. } => condition.validate(),
            Amount::Count(expression) => expression.validate(),
            Amount::Fixed(_) | Amount::X | Amount::CastCost(_) => Ok(()),
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
            Amount::CastCost(value) => {
                let mut variant = s.serialize_struct_variant("Amount", 2, "CastCost", 3)?;
                variant.serialize_field("cast_cost", &value.condition)?;
                variant.serialize_field("when_true", &value.if_selected)?;
                variant.serialize_field("otherwise", &value.otherwise)?;
                variant.end()
            }
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
    CastCost,
    Conditional,
    Count,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum ConditionalField {
    CastCost,
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
        let mut cast_cost = None;
        let mut when_true = None;
        let mut otherwise = None;
        while let Some(field) = map.next_key()? {
            match field {
                ConditionalField::CastCost => {
                    if cast_cost.is_some() {
                        return Err(serde::de::Error::duplicate_field("cast_cost"));
                    }
                    cast_cost = Some(map.next_value()?);
                }
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
        if let Some(condition_value) = cast_cost {
            if condition.is_some() {
                return Err(serde::de::Error::custom(
                    "amount cannot combine live and cast-cost conditions",
                ));
            }
            return Ok(Amount::CastCost(CastCostConditionalAmount {
                condition: condition_value,
                if_selected: when_true
                    .ok_or_else(|| serde::de::Error::missing_field("when_true"))?,
                otherwise: otherwise.ok_or_else(|| serde::de::Error::missing_field("otherwise"))?,
            }));
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
                    AmountVariant::CastCost => access.struct_variant(
                        &["cast_cost", "when_true", "otherwise"],
                        ConditionalAmountVisitor,
                    ),
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
