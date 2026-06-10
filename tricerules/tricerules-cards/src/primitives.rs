//! High-level spell effects referenced by `CardDefinition.spell_effect`.
//!
//! These are the generic, data-driven primitives of the hybrid card model: a
//! card's RON `spell_effect` deserializes straight into [`SpellEffectKind`]
//! (e.g. `DamageTarget(amount: 3, target: (kind: AnyTarget))`), so numeric
//! parameters and targeting live in card data, not in code.

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
    /// CR 702.18: this permanent can't be the target of spells or abilities your opponents control.
    Hexproof,
    /// CR 702.16: this permanent can't be the target of any spells or abilities (including yours).
    Shroud,
}

/// Base kind for a [`TargetFilter`] — what category of object is targeted.
/// The five values correspond to the original TargetSpec variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetKind {
    /// Creature or player (later expands to planeswalker/battle).
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

fn default_creature_filter() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::Creature,
        not_artifact: false,
        tapped: None,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetFilter {
    pub kind: TargetKind,
    /// If true, the target must not be an artifact.
    #[serde(default)]
    pub not_artifact: bool,
    /// If Some(true), target must be tapped; Some(false) must be untapped; None = either.
    #[serde(default)]
    pub tapped: Option<bool>,
}

impl TargetFilter {
    /// Default: any creature (the most common implicit filter).
    pub fn default_creature() -> Self {
        default_creature_filter()
    }

    /// True for player-only kinds (used by startup validation).
    pub fn is_player(&self) -> bool {
        matches!(
            self.kind,
            TargetKind::AnyPlayer | TargetKind::OpponentPlayer
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellEffectKind {
    DamageTarget {
        amount: u32,
        target: TargetFilter,
    },
    Draw {
        count: u32,
    },
    /// Destroy target matching `target` filter (default: any creature on the battlefield).
    DestroyTarget {
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
    },
    /// Destroy target tapped creature (e.g. Royal Assassin activated ability).
    DestroyTargetTapped,
    PumpTarget {
        power: i32,
        toughness: i32,
    },
    /// Tap target permanent matching `target` filter.
    TapTarget {
        target: TargetFilter,
    },
    CounterTargetSpell,
    GainLife {
        amount: u32,
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
    ReturnTargetCreatureToHand,
    ReturnTargetPermanentToHand,
    MillTargetPlayer {
        count: u32,
        target: TargetFilter,
    },
    None,
}

impl SpellEffectKind {
    /// Startup validation: reject effect/filter combinations the engine cannot honor.
    /// Returns `Err` with a human-readable reason; called from the card registry loader.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            SpellEffectKind::TargetPlayerGainsLife { target, .. }
            | SpellEffectKind::TargetPlayerLosesLife { target, .. }
            | SpellEffectKind::MillTargetPlayer { target, .. } => {
                if target.is_player() {
                    Ok(())
                } else {
                    Err(format!(
                        "player-targeted effect requires AnyPlayer or OpponentPlayer kind, got {:?}",
                        target.kind
                    ))
                }
            }
            SpellEffectKind::TapTarget { target } => {
                if target.is_player() {
                    Err(format!(
                        "TapTarget cannot target players, got {:?}",
                        target.kind
                    ))
                } else {
                    Ok(())
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
    /// Whenever a player casts a spell (optionally filtered by type). Parameters control
    /// whose casts qualify and which spell types count. Covers enchantress triggers
    /// (Argothian Enchantress), prowess-style draw/damage (Talrand, Young Pyromancer,
    /// Guttersnipe), and any-spell-cast watchers.
    WheneverPlayerCastsSpell {
        /// Whose casts trigger this ability relative to the source permanent's controller.
        /// Defaults to `Controller` ("whenever you cast").
        #[serde(default)]
        caster: CastTriggerPlayer,
        /// If `Some`, only spells of this type fire the trigger. `None` matches any spell.
        #[serde(default)]
        spell_type: Option<SpellTypeFilter>,
    },
}

/// Which player's spell casts trigger a `WheneverPlayerCastsSpell` ability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CastTriggerPlayer {
    /// "Whenever you cast" — only the controller of this permanent.
    #[default]
    Controller,
    /// "Whenever an opponent casts" — any player who is not the controller.
    Opponent,
    /// "Whenever a player casts" — any player including the controller.
    AnyPlayer,
}

/// Spell type filter for `WheneverPlayerCastsSpell`. `None` on the field means any type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellTypeFilter {
    Enchantment,
    Instant,
    Sorcery,
    /// Matches instants and sorceries (the most common pairing — Talrand, Young Pyromancer, etc.).
    InstantOrSorcery,
    Creature,
    Artifact,
    /// Matches any non-creature spell.
    Noncreature,
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

// ---------------------------------------------------------------------------
// Continuous effects (layer system, CR 613)
// ---------------------------------------------------------------------------

/// How long a continuous effect lasts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectDuration {
    /// Expires at the next cleanup step (CR 514.2).
    UntilEndOfTurn,
    // Future: WhileSourceOnBattlefield(ObjectId) — static abilities cleaned up at LTB
}

/// The kind of modification a continuous effect applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContinuousEffectKind {
    /// CR 613 layer 7c — modifying effects (+N/+N, -N/-N).
    PtModify {
        delta_power: i32,
        delta_toughness: i32,
    },
    // Future: Layer6AddKeyword(Keyword), Layer7bSetPt { power: i32, toughness: i32 }, …
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_effect_accepts_player_spec() {
        assert!(SpellEffectKind::TargetPlayerLosesLife {
            amount: 3,
            target: TargetFilter {
                kind: TargetKind::OpponentPlayer,
                not_artifact: false,
                tapped: None,
            },
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn player_effect_rejects_nonplayer_spec() {
        assert!(SpellEffectKind::TargetPlayerGainsLife {
            amount: 3,
            target: TargetFilter {
                kind: TargetKind::Creature,
                not_artifact: false,
                tapped: None,
            },
        }
        .validate()
        .is_err());
    }

    #[test]
    fn damage_accepts_any_kind() {
        for kind in [
            TargetKind::AnyTarget,
            TargetKind::Creature,
            TargetKind::AnyPlayer,
            TargetKind::OpponentPlayer,
        ] {
            assert!(SpellEffectKind::DamageTarget {
                amount: 3,
                target: TargetFilter {
                    kind,
                    not_artifact: false,
                    tapped: None
                },
            }
            .validate()
            .is_ok());
        }
    }
}
