//! Target kinds and composable target filters.

use super::{Color, Keyword, SpellEffectKind};
use serde::{Deserialize, Serialize};

/// Authored target declaration for one spell, mode, activated ability, or triggered ability.
/// Omit it to retain the legacy single required group containing every targeted effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetingDef {
    pub groups: Vec<TargetGroupDef>,
}

/// One independently prompted target group. `effect_indices` names the effects in the sibling
/// effect list that consume this group's ordered targets. Candidate publication intersects the
/// requirements of every referenced effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetGroupDef {
    pub min: u32,
    pub max: u32,
    pub prompt: String,
    pub effect_indices: Vec<u32>,
    /// Indices of other groups whose chosen objects must be distinct from this group.
    #[serde(default)]
    pub distinct_from: Vec<u32>,
}

impl TargetingDef {
    pub(crate) fn validate(&self, effects: &[SpellEffectKind]) -> Result<(), String> {
        if self.groups.is_empty() {
            return Err("targeting requires at least one group".into());
        }
        let mut referenced = vec![0u32; effects.len()];
        for (group_index, group) in self.groups.iter().enumerate() {
            if group.min > group.max || group.max == 0 {
                return Err("target group requires min <= max and max > 0".into());
            }
            if group.prompt.trim().is_empty() {
                return Err("target group prompt must not be empty".into());
            }
            if group.effect_indices.is_empty() {
                return Err("target group must reference at least one effect".into());
            }
            let mut within_group = std::collections::HashSet::new();
            for &effect_index in &group.effect_indices {
                let effect = effects
                    .get(effect_index as usize)
                    .ok_or_else(|| "target group references an unknown effect".to_string())?;
                if !within_group.insert(effect_index) {
                    return Err("target group references an effect more than once".into());
                }
                if !effect.needs_target() {
                    return Err("target group references an untargeted effect".into());
                }
                referenced[effect_index as usize] += 1;
            }
            let mut distinct = std::collections::HashSet::new();
            for &other in &group.distinct_from {
                if other as usize >= self.groups.len() || other as usize == group_index {
                    return Err("target group has an invalid distinctness reference".into());
                }
                if !distinct.insert(other) {
                    return Err("target group repeats a distinctness reference".into());
                }
            }
        }
        for (index, effect) in effects.iter().enumerate() {
            let expected = if effect.needs_target() {
                effect.target_filters().len().max(1) as u32
            } else {
                0
            };
            if referenced[index] != expected {
                return Err(format!(
                    "targeted effect {index} must belong to exactly {expected} target group(s)"
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_optional(
        targeting: Option<&Self>,
        effects: &[SpellEffectKind],
    ) -> Result<(), String> {
        if let Some(targeting) = targeting {
            return targeting.validate(effects);
        }
        if effects
            .iter()
            .any(|effect| effect.target_filters().len() > 1)
        {
            return Err("an effect with multiple target roles requires grouped targeting".into());
        }
        Ok(())
    }
}

/// Base kind for a [`TargetFilter`] — what category of object is targeted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TargetKind {
    /// Creature or player (later expands to planeswalker/battle).
    #[default]
    AnyTarget,
    /// A creature on the battlefield.
    Creature,
    /// Any player still in the game, including the caster.
    AnyPlayer,
    /// Any player still in the game except the caster.
    OpponentPlayer,
    /// Any permanent on the battlefield (artifact, creature, or land).
    AnyPermanent,
}

/// Card-type predicate shared by spells on the stack and cards in other zones. `None` on a
/// containing field means no type restriction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardTypeFilter {
    Enchantment,
    Instant,
    Sorcery,
    /// Matches instants and sorceries (Talrand, Young Pyromancer, Mystical Tutor).
    InstantOrSorcery,
    Creature,
    Artifact,
    /// Matches any object that does not have the creature card type.
    Noncreature,
}

/// Controller relationship required of a permanent target, relative to the spell or ability
/// controller. This remains independently composable with the other `TargetFilter` predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TargetController {
    /// A permanent controlled by any player.
    #[default]
    Any,
    /// A permanent controlled by the spell or ability's controller ("you control").
    You,
    /// A permanent controlled by an opponent of the spell or ability's controller.
    Opponent,
    /// A permanent not controlled by the spell or ability's controller. Unlike `Opponent`, this
    /// remains correct for teammates in player-set-aware formats.
    NotYou,
}

/// Inclusive comparison against a permanent's current derived power.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerComparison {
    AtLeast(u32),
    AtMost(u32),
}

/// Which player's graveyard a [`GraveyardFilter`] targets. Defaults to [`Controller`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GraveyardOwner {
    /// Only the effect controller's own graveyard ("your graveyard" — Raise Dead, Disentomb).
    #[default]
    Controller,
    /// Any player's graveyard ("a graveyard" — Grim Return, Beacon of Unrest).
    AnyPlayer,
}

/// Filter for graveyard-zone targets (cards in a graveyard, not battlefield permanents).
/// Parallel to [`TargetFilter`] but for a different zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GraveyardFilter {
    /// Which player's graveyard. Defaults to the caster's own graveyard.
    #[serde(default)]
    pub owner: GraveyardOwner,
    /// Optional card-type restriction. `None` = any card.
    #[serde(default)]
    pub card_type: Option<CardTypeFilter>,
}

/// Where a card returned from the graveyard lands (CR 400.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraveyardDestination {
    /// The card goes to its owner's hand (Raise Dead, Disentomb, Gravedigger ETB).
    Hand,
    /// The card enters the battlefield under the caster's control (reanimation spells).
    Battlefield,
}

fn default_creature_filter() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::Creature,
        ..TargetFilter::default()
    }
}

/// Composable target predicate: base [`TargetKind`] AND optional characteristic
/// constraints (AND-combined). Use only `kind` to get the same semantics as the
/// original five TargetSpec variants; add constraints to narrow further.
///
/// Example RON:
/// - `(kind: AnyTarget)` — any creature or player
/// - `(kind: Creature, not_artifact: true)` — non-artifact creature
/// - `(kind: Creature, tapped: true)` — tapped creature (for future use)
/// - `(kind: Creature, not_color: Black)` — nonblack creature (Doom Blade, Terror)
/// - `(kind: Creature, is_color: Green)` — green creature (Perish, Virtue's Ruin)
/// - `(kind: Creature, attacking_or_blocking: true)` — Divine Verdict, Hunt Down
/// - `(kind: Creature, controller: You)` — "target creature you control" (Equip, Regenerate,
///   many activated abilities). Enforced at targeting time relative to the activating player.
/// - `(kind: AnyPermanent, controller: Opponent, permanent_types: [Artifact, Enchantment])` —
///   Rambunctious Mutt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TargetFilter {
    #[serde(default)]
    pub kind: TargetKind,
    /// If true, the target must not be an artifact.
    #[serde(default)]
    pub not_artifact: bool,
    /// If Some(true), target must be tapped; Some(false) must be untapped; None = either.
    #[serde(default)]
    pub tapped: Option<bool>,
    /// CR 508/509: if true, the target must currently be attacking or blocking. Combat-only
    /// removal/tricks — Divine Verdict, Hunt Down ("destroy target attacking or blocking creature").
    #[serde(default)]
    pub attacking_or_blocking: bool,
    /// CR 105/202.2: if `Some`, the target must NOT be of this color (derived from its mana cost).
    /// Doom Blade ("nonblack creature"), Terror ("nonblack" — paired with `not_artifact`).
    #[serde(default)]
    pub not_color: Option<Color>,
    /// CR 105/202.2: if `Some`, the object must BE of this color — the inclusive mirror of
    /// `not_color`. Perish ("all green creatures") and Virtue's Ruin ("all white creatures") on
    /// the untargeted side; Hydroblast / Pyroblast ("target red permanent") on the targeted side.
    #[serde(default)]
    pub is_color: Option<Color>,
    /// Controller relationship for a permanent target, relative to the spell or ability's
    /// controller. Covers "you control" (Equipment, regeneration) and "an opponent controls"
    /// (Glaring Aegis, Rambunctious Mutt) while composing with every other filter field.
    #[serde(default)]
    pub controller: TargetController,
    /// If true, the object that sourced this spell or ability is not a legal target. The engine
    /// compares full object identity (ObjectId plus zone-change generation), so a card that leaves
    /// and returns is a new object under CR 400.7 and is no longer excluded.
    #[serde(default)]
    pub exclude_source: bool,
    /// Optional OR-combined permanent type restriction. For example, Icy Manipulator uses
    /// `[Artifact, Creature, Land]`; an empty list means no additional type restriction.
    #[serde(default)]
    pub permanent_types: Vec<super::PermanentTypeFilter>,
    /// Subtypes that disqualify the target (for example, Eyeblight's Ending's "non-Elf"
    /// restriction). Empty means no excluded subtypes.
    #[serde(default)]
    pub excluded_subtypes: Vec<String>,
    /// Optional comparison against current derived power. A noncreature has no power and cannot
    /// match a power-constrained filter.
    #[serde(default)]
    pub power: Option<PowerComparison>,
    /// Keywords every matching permanent must currently have. Shared by targeted, untargeted,
    /// and cost-selection predicates (for example Defender on Portcullis Vine / Run Afoul).
    #[serde(default)]
    pub required_keywords: Vec<Keyword>,
    /// Keywords every matching permanent must currently lack. This is the negative counterpart
    /// to `required_keywords` and is shared by targets and untargeted selections.
    #[serde(default)]
    pub excluded_keywords: Vec<Keyword>,
}

impl TargetFilter {
    /// Default: any creature (the most common implicit filter).
    pub fn default_creature() -> Self {
        default_creature_filter()
    }

    /// Default filter for the equip ability: "target creature you control" (CR 702.6a).
    pub fn default_equip() -> Self {
        TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            ..TargetFilter::default()
        }
    }

    /// Validate constraints that depend on the filter representing an actual target. Player and
    /// `AnyTarget` filters cannot carry a permanent-controller predicate, while source exclusion
    /// is meaningful for every object-capable kind (including `AnyTarget`) but never player-only
    /// kinds.
    pub(crate) fn validate_target_constraints(&self) -> Result<(), String> {
        self.validate_characteristic_constraints()?;
        if self.controller != TargetController::Any
            && !matches!(self.kind, TargetKind::Creature | TargetKind::AnyPermanent)
        {
            return Err(format!(
                "controller-relative target filter requires Creature or AnyPermanent kind, got {:?}",
                self.kind
            ));
        }
        if self.exclude_source && self.is_player() {
            return Err(format!(
                "source-excluding target filter requires an object-capable kind, got {:?}",
                self.kind
            ));
        }
        Ok(())
    }

    /// Reject contradictory characteristic predicates wherever this filter is used, including
    /// untargeted mass effects and sacrifice selections.
    pub(crate) fn validate_characteristic_constraints(&self) -> Result<(), String> {
        if let Some(keyword) = self
            .required_keywords
            .iter()
            .find(|keyword| self.excluded_keywords.contains(keyword))
        {
            return Err(format!(
                "target filter cannot both require and exclude keyword {keyword:?}"
            ));
        }
        Ok(())
    }

    /// True for player-only kinds (used by startup validation).
    pub fn is_player(&self) -> bool {
        matches!(
            self.kind,
            TargetKind::AnyPlayer | TargetKind::OpponentPlayer
        )
    }
}
