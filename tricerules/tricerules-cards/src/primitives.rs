//! High-level spell effects referenced by `CardDefinition.spell_effect`.
//!
//! These are the generic, data-driven primitives of the hybrid card model: a
//! card's RON `spell_effect` deserializes straight into [`SpellEffectKind`]
//! (e.g. `DamageTarget(amount: 3, target: AnyTarget)`), so numeric parameters
//! and targeting live in card data, not in code.

use serde::{Deserialize, Serialize};

/// The five MTG colors. Used for characteristic-based blocking checks (Intimidate, Protection)
/// and derived from a card's mana cost at query time — not stored as a separate RON field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    White,
    Blue,
    Black,
    Red,
    Green,
}

/// Static keyword abilities that affect game rules (blocking restrictions, attack
/// rules, damage modifiers, etc.). Parameterless only — parameterized keywords
/// (e.g. Protection from X, Landwalk) are deferred to the custom-Rust tier since
/// they require characteristic matching the data-driven tier can't express.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Keyword {
    /// CR 702.9: this creature can only be blocked by creatures with flying or reach.
    Flying,
    /// CR 702.17: this creature can block creatures with flying.
    Reach,
    /// CR 702.13: this creature can only be blocked by artifact creatures and/or
    /// creatures that share a color with it.
    Intimidate,
    /// CR 702.20: this creature doesn't tap when it attacks.
    Vigilance,
    /// CR 702.15: damage dealt by this permanent also causes its controller to gain that much life.
    Lifelink,
}

/// What a single target must be. Deliberately a small, flat enum covering only
/// the distinctions the engine makes today; richer characteristic-based filters
/// (creature type, color, etc.) are deferred to the future Rust scripting tier
/// rather than modeled here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetSpec {
    /// Creature or player (later expands to planeswalker/battle).
    AnyTarget,
    /// A creature on the battlefield.
    Creature,
    /// Any player still in the game, including the caster.
    AnyPlayer,
    /// Any player still in the game except the caster.
    OpponentPlayer,
}

impl TargetSpec {
    /// True for the player-only specs (used by startup validation).
    pub fn is_player(&self) -> bool {
        matches!(self, TargetSpec::AnyPlayer | TargetSpec::OpponentPlayer)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellEffectKind {
    DamageTarget { amount: u32, target: TargetSpec },
    Draw { count: u32 },
    DestroyTarget,
    PumpTarget { power: i32, toughness: i32 },
    CounterTargetSpell,
    GainLife { amount: u32 },
    TargetPlayerGainsLife { amount: u32, target: TargetSpec },
    TargetPlayerLosesLife { amount: u32, target: TargetSpec },
    EachOpponentLosesLifeYouGainEqual { amount: u32 },
    ExileTarget,
    ExileTargetGainLifeEqualToPower,
    ReturnTargetCreatureToHand,
    ReturnTargetPermanentToHand,
    MillTargetPlayer { count: u32, target: TargetSpec },
    None,
}

impl SpellEffectKind {
    /// Startup validation: reject effect/`TargetSpec` combinations the engine
    /// cannot honor (e.g. a player-life effect pointed at a creature). Returns
    /// `Err` with a human-readable reason; called from the card registry loader.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            SpellEffectKind::TargetPlayerGainsLife { target, .. }
            | SpellEffectKind::TargetPlayerLosesLife { target, .. }
            | SpellEffectKind::MillTargetPlayer { target, .. } => {
                if target.is_player() {
                    Ok(())
                } else {
                    Err(format!(
                        "player-targeted effect requires AnyPlayer or OpponentPlayer, got {target:?}"
                    ))
                }
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_effect_accepts_player_spec() {
        assert!(SpellEffectKind::TargetPlayerLosesLife {
            amount: 3,
            target: TargetSpec::OpponentPlayer,
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn player_effect_rejects_nonplayer_spec() {
        assert!(SpellEffectKind::TargetPlayerGainsLife {
            amount: 3,
            target: TargetSpec::Creature,
        }
        .validate()
        .is_err());
    }

    #[test]
    fn damage_accepts_any_spec() {
        for spec in [
            TargetSpec::AnyTarget,
            TargetSpec::Creature,
            TargetSpec::AnyPlayer,
            TargetSpec::OpponentPlayer,
        ] {
            assert!(SpellEffectKind::DamageTarget {
                amount: 3,
                target: spec,
            }
            .validate()
            .is_ok());
        }
    }
}
