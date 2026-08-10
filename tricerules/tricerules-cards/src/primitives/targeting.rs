//! Target kinds and composable target filters.

use super::Color;
use serde::{Deserialize, Serialize};

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

/// Which card types in a graveyard qualify for [`ReturnFromGraveyard`][SpellEffectKind::ReturnFromGraveyard].
/// `None` means any card type (no type restriction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraveyardCardType {
    /// Only creature cards (Raise Dead, Disentomb, Gravedigger ETB).
    Creature,
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
    pub card_type: Option<GraveyardCardType>,
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
    /// Optional OR-combined permanent type restriction. For example, Icy Manipulator uses
    /// `[Artifact, Creature, Land]`; an empty list means no additional type restriction.
    #[serde(default)]
    pub permanent_types: Vec<super::PermanentTypeFilter>,
    /// Subtypes that disqualify the target (for example, Eyeblight's Ending's "non-Elf"
    /// restriction). Empty means no excluded subtypes.
    #[serde(default)]
    pub excluded_subtypes: Vec<String>,
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

    /// Validate a controller relationship used for actual targeting. Player and `AnyTarget`
    /// filters cannot carry a permanent-controller predicate.
    pub(crate) fn validate_target_controller(&self) -> Result<(), String> {
        if self.controller != TargetController::Any
            && !matches!(self.kind, TargetKind::Creature | TargetKind::AnyPermanent)
        {
            return Err(format!(
                "controller-relative target filter requires Creature or AnyPermanent kind, got {:?}",
                self.kind
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
