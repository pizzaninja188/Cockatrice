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
    /// The source permanent itself. **Not "targeting" in the CR sense** (CR 115): it is
    /// auto-bound to the ability's source, never a player choice, and ignores hexproof/shroud.
    /// Legal only inside an activated or triggered ability effect, never in `spell_effect`
    /// (enforced by [`SpellEffectKind::validate`]). Replaces the old `TriggeredEffect::PumpSelf`.
    Self_,
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
/// - `(kind: Creature, attacking_or_blocking: true)` — Divine Verdict, Hunt Down
/// - `(kind: Creature, only_controller: true)` — "target creature you control" (Equip,
///   Regenerate, many activated abilities). Enforced at targeting time; the controller
///   is the activating player.
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
    /// "target creature you control" restriction (CR 702.6a / 701.15 regenerate / various
    /// activated abilities). The target must be owned/controlled by the activating player.
    /// Covers Equipment equip (Bonesplitter, Vulshok Morningstar) and Regenerate (Drudge
    /// Skeletons, Cudgel Troll) without a new variant.
    #[serde(default)]
    pub only_controller: bool,
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
            only_controller: true,
            ..TargetFilter::default()
        }
    }

    /// True for player-only kinds (used by startup validation).
    pub fn is_player(&self) -> bool {
        matches!(
            self.kind,
            TargetKind::AnyPlayer | TargetKind::OpponentPlayer
        )
    }
}
