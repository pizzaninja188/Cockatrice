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
    /// CR 702.10: this creature is unaffected by summoning sickness — it can attack and use
    /// activated abilities that include {T} or {Q} even on the turn it entered the battlefield.
    Haste,
    /// CR 702.2: any amount of damage this creature deals to another creature is enough to
    /// destroy it (as a state-based action; CR 704.5h).
    Deathtouch,
    /// CR 702.110: this creature can't be blocked except by two or more creatures.
    Menace,
    /// CR 702.19: if this creature is blocked, excess combat damage (above lethal for all blockers)
    /// is assigned to the defending player rather than being lost.
    Trample,
    /// CR 702.7: this creature assigns its combat damage in the first combat damage step
    /// (CR 510.5); creatures without first strike or double strike wait until the regular step.
    FirstStrike,
    /// CR 702.4: this creature assigns combat damage in both combat damage steps (CR 510.5):
    /// the first-strike step (like first strike) and the regular step (like a vanilla creature).
    DoubleStrike,
    /// CR 702.12: this permanent can't be destroyed by lethal damage or "destroy" effects.
    /// It still dies if its toughness drops to 0 (CR 704.5f).
    Indestructible,
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
    /// Any permanent on the battlefield (artifact, creature, or land — e.g. Icy Manipulator).
    AnyPermanent,
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
    /// Destroy target tapped creature (e.g. Royal Assassin activated ability).
    DestroyTargetTapped,
    PumpTarget { power: i32, toughness: i32 },
    /// Tap target permanent (artifact, creature, or land — e.g. Icy Manipulator).
    TapTarget { target: TargetSpec },
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
            SpellEffectKind::TapTarget { target } => {
                if matches!(target, TargetSpec::AnyPermanent) {
                    Ok(())
                } else {
                    Err(format!(
                        "TapTarget requires AnyPermanent, got {target:?}"
                    ))
                }
            }
            _ => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------
// Activated abilities
// ---------------------------------------------------------------------------

/// Cost to activate an activated ability (CR 602).
/// Mana abilities that only produce mana (CR 605.3) are intentionally excluded —
/// they don't use the stack and are handled separately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilityCost {
    /// {T}: tap the source permanent.
    Tap,
    /// Pay mana (e.g. "4", "2R"). Uses the same minimal mana string as `CardDefinition.mana_cost`.
    Mana(String),
    /// {T} plus mana (e.g. Jayemdae Tome: "4" + tap).
    TapAndMana(String),
    /// Sacrifice the source permanent as cost (e.g. Bottle Gnomes).
    Sacrifice,
}

/// One activated ability on a permanent (RON data tier). Cost + effect compose freely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivatedAbilityDef {
    pub cost: AbilityCost,
    pub effect: SpellEffectKind,
    /// Oracle-style ability text shown as annotation on the stack card.
    pub text: String,
}

// ---------------------------------------------------------------------------
// Triggered abilities
// ---------------------------------------------------------------------------

/// Condition that causes a triggered ability to fire (CR 603).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerCondition {
    /// When this permanent enters the battlefield.
    WhenSelfEntersBattlefield,
    /// When this permanent is put into a graveyard from the battlefield.
    WhenSelfDies,
    /// Whenever this creature attacks.
    WheneverSelfAttacks,
    /// Whenever this creature deals combat damage to a player (e.g. Scroll Thief).
    WheneverSelfDealsCombatDamageToPlayer,
    /// Whenever this creature deals damage to an opponent, combat or non-combat (e.g. Thieving Magpie).
    WheneverSelfDealsDamageToOpponent,
    /// At the beginning of this permanent's controller's upkeep.
    AtBeginningOfControllerUpkeep,
}

/// Effect of a triggered ability. Wraps `SpellEffectKind` for the common case, plus
/// self-referential effects that don't map to a targeted spell effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggeredEffect {
    /// Delegate to the existing spell-effect resolution path.
    Effect(SpellEffectKind),
    /// Source permanent gets +power/+toughness until end of turn.
    PumpSelf { power: i32, toughness: i32 },
}

/// One triggered ability on a permanent (RON data tier).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggeredAbilityDef {
    pub trigger: TriggerCondition,
    pub effect: TriggeredEffect,
    /// Oracle-style ability text shown as annotation on the stack card.
    pub text: String,
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
