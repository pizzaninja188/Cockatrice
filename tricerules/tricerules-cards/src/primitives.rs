//! High-level spell effects referenced by `CardDefinition.spell_effect`.
//!
//! These are the generic, data-driven primitives of the hybrid card model: a
//! card's RON `spell_effect` deserializes straight into [`SpellEffectKind`]
//! (e.g. `DamageTarget(amount: 3, target: (kind: AnyTarget))`), so numeric
//! parameters and targeting live in card data, not in code.

use crate::mana::ManaCost;
use serde::{Deserialize, Serialize};
use std::fmt;

/// An effect amount that is either a fixed literal or the spell's cast-time X (CR 107.3).
///
/// In RON a bare integer (`amount: 3`) is [`Amount::Fixed`]; the string `amount: "X"` is the
/// chosen X, resolved from the resolving stack item's `chosen_x`. Custom (de)serialize keeps the
/// existing integer corpus untouched and roundtrips X as the string `"X"` (RON renders a bare
/// `X` identifier as an ambiguous unit value, so the quoted form is used). Applied to the
/// amount-bearing effects that can legally scale with X — the "name two cards" pair is Fireball
/// (`DamageTarget { amount: "X" }`) and Blue Sun's Zenith (`Draw { count: "X" }`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Amount {
    /// A literal count baked into the card data.
    Fixed(u32),
    /// The spell's cast-time X value (CR 107.3); resolved at resolution from `chosen_x`.
    X,
}

impl Amount {
    /// Resolve to a concrete count given the spell's chosen X (0 for non-X spells).
    pub fn resolve(self, x: u32) -> u32 {
        match self {
            Amount::Fixed(n) => n,
            Amount::X => x,
        }
    }

    /// True if this amount depends on the cast-time X.
    pub fn is_x(self) -> bool {
        matches!(self, Amount::X)
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
        }
    }
}

impl<'de> Deserialize<'de> for Amount {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct AmountVisitor;
        impl serde::de::Visitor<'_> for AmountVisitor {
            type Value = Amount;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a non-negative integer or the string \"X\"")
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
        }
        d.deserialize_any(AmountVisitor)
    }
}

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
    /// CR 702.111: this creature can't be blocked except by two or more creatures.
    Menace,
    /// CR 702.19: if this creature is blocked, excess combat damage (above lethal for all blockers)
    /// is assigned to the defending player rather than being lost.
    Trample,
    /// CR 702.7: this creature assigns its combat damage in the first combat damage step
    /// (CR 510.4); creatures without first strike or double strike wait until the regular step.
    FirstStrike,
    /// CR 702.4: this creature assigns combat damage in both combat damage steps (CR 510.4):
    /// the first-strike step (like first strike) and the regular step (like a vanilla creature).
    DoubleStrike,
    /// CR 702.12: this permanent can't be destroyed by lethal damage or "destroy" effects.
    /// It still dies if its toughness drops to 0 (CR 704.5f).
    Indestructible,
    /// CR 702.18: this permanent can't be the target of spells or abilities your opponents control.
    Hexproof,
    /// CR 702.16: this permanent can't be the target of any spells or abilities (including yours).
    Shroud,
    /// CR 702.3: this creature can't attack. (Walls and other defensive creatures.)
    Defender,
    /// CR 702.8: this card may be cast any time its controller could cast an instant
    /// (CR 601 + 702.8b), overriding the normal sorcery-speed restriction on permanents.
    Flash,
}

impl Keyword {
    /// Canonical MTG keyword text (e.g. `Keyword::FirstStrike` → `"First strike"`). Used for the
    /// token identity feed and any place a keyword must render as printed Oracle text.
    pub fn as_str(self) -> &'static str {
        match self {
            Keyword::Flying => "Flying",
            Keyword::Reach => "Reach",
            Keyword::Intimidate => "Intimidate",
            Keyword::Vigilance => "Vigilance",
            Keyword::Lifelink => "Lifelink",
            Keyword::Haste => "Haste",
            Keyword::Deathtouch => "Deathtouch",
            Keyword::Menace => "Menace",
            Keyword::Trample => "Trample",
            Keyword::FirstStrike => "First strike",
            Keyword::DoubleStrike => "Double strike",
            Keyword::Indestructible => "Indestructible",
            Keyword::Hexproof => "Hexproof",
            Keyword::Shroud => "Shroud",
            Keyword::Defender => "Defender",
            Keyword::Flash => "Flash",
        }
    }
}

/// A kind of counter that can sit on a permanent (CR 122.1). Only the two counter kinds
/// with engine rules interactions exist so far: the +1/+1 / -1/-1 pair, which modify P/T in
/// CR 613.4 layer 7d and annihilate as a state-based action (CR 122.3). Loyalty, charge, and
/// keyword counters are added by their dependent plans (planeswalkers, Chalice-style chargers)
/// when the first card needs them. `Ord` is required so [`crate`] consumers can store counters
/// in a `BTreeMap` for deterministic iteration/serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CounterKind {
    /// CR 122 +1/+1 counter — adds 1 to power and toughness each (layer 7d).
    PlusOnePlusOne,
    /// CR 122 -1/-1 counter — subtracts 1 from power and toughness each (layer 7d).
    MinusOneMinusOne,
}

impl CounterKind {
    /// Short human-readable label for client display (e.g. in card annotations).
    /// Matches the conventional MTG counter naming ("+1/+1", "-1/-1").
    pub fn label(self) -> &'static str {
        match self {
            CounterKind::PlusOnePlusOne => "+1/+1",
            CounterKind::MinusOneMinusOne => "-1/-1",
        }
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
    /// The source permanent itself. **Not "targeting" in the CR sense** (CR 115): it is
    /// auto-bound to the ability's source, never a player choice, and ignores hexproof/shroud.
    /// Legal only inside an activated or triggered ability effect, never in `spell_effect`
    /// (enforced by [`SpellEffectKind::validate`]). Replaces the old `TriggeredEffect::PumpSelf`.
    Self_,
}

/// Where an effect is being resolved from. Controls validation that depends on context —
/// e.g. [`TargetKind::Self_`] is only meaningful for an ability bound to a source permanent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectContext {
    /// A spell's `spell_effect` list (no source permanent to self-reference).
    Spell,
    /// An activated or triggered ability bound to a source permanent.
    Ability,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellEffectKind {
    DamageTarget {
        amount: Amount,
        target: TargetFilter,
    },
    Draw {
        count: Amount,
    },
    /// Destroy target matching `target` filter (default: any creature on the battlefield).
    /// Characteristic restrictions (e.g. `tapped: true` for Royal Assassin) live in the filter.
    DestroyTarget {
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
    },
    /// Give +power/+toughness until end of turn to a creature matching `target`
    /// (default: any creature, for Giant Growth). Use `(kind: Self_)` for an ability that
    /// pumps its own source permanent (e.g. an upkeep self-pump) — auto-bound, untargeted.
    PumpTarget {
        power: i32,
        toughness: i32,
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
    },
    /// Tap target permanent matching `target` filter.
    TapTarget {
        target: TargetFilter,
    },
    /// CR 701.5: counter target spell on the stack. `spell_filter` narrows which spells are legal
    /// targets — `None` is unrestricted (Counterspell), `Some(Creature)` is Essence Scatter,
    /// `Some(Noncreature)` is Negate. Reuses [`SpellTypeFilter`] so any future "counter target
    /// X spell" needs no new variant.
    CounterTargetSpell {
        #[serde(default)]
        spell_filter: Option<SpellTypeFilter>,
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
        spell_filter: Option<SpellTypeFilter>,
    },
    /// CR 613.4 layer 7c: give every creature matching `filter` +power/+toughness until end of
    /// turn (the mass, one-shot sibling of [`Self::PumpTarget`]). Untargeted — `filter` selects
    /// the set the same way a static anthem does. Glorious Charge / Inspired Charge
    /// (`controller: YouControl`); attacking-creature pumps reuse the same filter machinery.
    PumpAll {
        #[serde(default)]
        filter: AnthemFilter,
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
    /// [`StaticAbilityDef::AnthemKeyword`]. Covers Overrun (Trample to all your creatures) and
    /// Trumpet Blast (First Strike to attacking creatures you control until EOT).
    GrantKeywordsAll {
        #[serde(default)]
        filter: AnthemFilter,
        keywords: Vec<Keyword>,
    },
    GainLife {
        amount: Amount,
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
    /// Destroy every battlefield permanent matching `kind` (CR 701.7). Untargeted, so it
    /// ignores hexproof/shroud and never fizzles. `kind` selects the affected set — `Creature`
    /// for Wrath of God / Day of Judgment, `AnyPermanent` for "destroy all permanents". Only
    /// object kinds are legal (validated at load); player kinds make no sense here.
    /// `prevent_regeneration: true` means regeneration shields are bypassed (Wrath of God:
    /// "they can't be regenerated", CR 701.15b).
    DestroyAll {
        #[serde(default = "TargetFilter::default_creature")]
        kind: TargetFilter,
        #[serde(default)]
        prevent_regeneration: bool,
    },
    /// CR 701.15: put a regeneration shield on target creature. The next time that creature would
    /// be destroyed this turn, instead tap it, remove it from combat, and clear all damage from it.
    /// Legal only as an activated ability effect — never a spell (validated at load). Covers
    /// Cudgel Troll (`{G}: Regenerate`) and Drudge Skeletons (`{B}: Regenerate`).
    Regenerate {
        #[serde(default = "TargetFilter::default_creature")]
        target: TargetFilter,
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
    /// [`TokenDefinition`](crate::token_def::TokenDefinition); only `count` and `controller` vary
    /// per maker. Covers Raise the Alarm / Dragon Fodder (`Controller`, count 2) and symmetrical
    /// makers (`EachPlayer`). Token *copies* of existing permanents (CR 707) are a separate effect.
    CreateTokens {
        /// Token id (slug of the token's name) in the registry's token namespace.
        token: String,
        count: u32,
        #[serde(default)]
        controller: TokenController,
    },
    /// CR 122/121.6: put `count` counters of `counter` on a creature matching `target`
    /// (default: any creature). The `counter` kind covers both +1/+1 counter spells
    /// (Battlegrowth, Common Bond) and -1/-1 counter spells (Instill Infection) without a new
    /// variant. Use `(kind: Self_)` for an ability that puts counters on its own source
    /// (modular/graft/outlast self-buffs). Counter *removal* spells are deferred — counter
    /// removal in MTG is almost always an ability cost (see the plan's `AbilityCost` phase).
    PutCounters {
        counter: CounterKind,
        count: u32,
        #[serde(default = "TargetFilter::default_creature")]
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
    },
    /// CR 301.5 / 702.6: the equip activated ability — attach this equipment to `target` creature
    /// you control. At resolution the engine moves `attached_to` on the equipment's `GameObject`
    /// to the new target (detaching from any previous creature automatically). The P/T bonus
    /// (if any) is a separate [`StaticAbilityDef::EquippedBonus`] that reads `attached_to`
    /// dynamically, so no continuous effect is updated on re-equip. Legal only as an activated
    /// ability's `effect`, never a spell effect; equip only as a sorcery (CR 702.6a).
    /// Covers Bonesplitter (equip {1}) and Vulshok Morningstar (equip {2}).
    Equip {
        #[serde(default = "TargetFilter::default_equip")]
        target: TargetFilter,
    },
    None,
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

/// Who receives the tokens made by [`SpellEffectKind::CreateTokens`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TokenController {
    /// The spell/ability's controller (the common case: Raise the Alarm, Krenko).
    #[default]
    Controller,
    /// Each player still in the game gets `count` tokens (symmetrical makers).
    EachPlayer,
}

impl SpellEffectKind {
    /// The target filter(s) this effect selects against, if any. Used by validation and by
    /// the engine's generic legality/targeting paths (one place to enumerate target-bearing
    /// variants instead of repeating the list).
    pub fn target_filters(&self) -> Vec<&TargetFilter> {
        match self {
            SpellEffectKind::DamageTarget { target, .. }
            | SpellEffectKind::DestroyTarget { target }
            | SpellEffectKind::PumpTarget { target, .. }
            | SpellEffectKind::TapTarget { target }
            | SpellEffectKind::TargetPlayerGainsLife { target, .. }
            | SpellEffectKind::TargetPlayerLosesLife { target, .. }
            | SpellEffectKind::MillTargetPlayer { target, .. }
            | SpellEffectKind::PutCounters { target, .. }
            | SpellEffectKind::AuraAttach { target }
            | SpellEffectKind::Equip { target }
            | SpellEffectKind::Regenerate { target } => vec![target],
            _ => vec![],
        }
    }

    /// Startup validation: reject effect/filter combinations the engine cannot honor.
    /// Returns `Err` with a human-readable reason; called from the card registry loader.
    /// `context` distinguishes spells from abilities so context-only filters (`Self_`) are
    /// rejected where they make no sense.
    pub fn validate(&self, context: EffectContext) -> Result<(), String> {
        // CR 115: a self-referencing ability effect is not "targeting" and only exists where
        // there is a source permanent — never in a spell's effect list.
        if context == EffectContext::Spell
            && self
                .target_filters()
                .iter()
                .any(|f| f.kind == TargetKind::Self_)
        {
            return Err(
                "Self_ target is only valid on an activated or triggered ability, not a spell"
                    .into(),
            );
        }
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
            // CR 122: counters go on permanents, never players.
            SpellEffectKind::PutCounters { target, .. } => {
                if target.is_player() {
                    Err(format!(
                        "PutCounters cannot target players, got {:?}",
                        target.kind
                    ))
                } else {
                    Ok(())
                }
            }
            // Mass effects select objects, not players, and never use Self_/AnyTarget (which
            // include players). Only Creature / AnyPermanent are honored by the engine.
            SpellEffectKind::DestroyAll { kind, .. } | SpellEffectKind::DamageAll { kind, .. } => {
                if matches!(kind.kind, TargetKind::Creature | TargetKind::AnyPermanent) {
                    Ok(())
                } else {
                    Err(format!(
                        "mass effect kind must be Creature or AnyPermanent, got {:?}",
                        kind.kind
                    ))
                }
            }
            // CR 605.1a: a mana ability is an activated/triggered ability — never a spell. An
            // empty option set would produce nothing and is rejected as malformed.
            SpellEffectKind::ProduceMana { options } => {
                if context == EffectContext::Spell {
                    Err("ProduceMana is only valid on a mana ability, not a spell".into())
                } else if options.is_empty() {
                    Err("ProduceMana requires at least one mana option".into())
                } else {
                    Ok(())
                }
            }
            // CR 303.4a: an aura enchants a permanent (never a player).
            SpellEffectKind::AuraAttach { target } => {
                if target.is_player() {
                    Err(
                        "AuraAttach cannot target players; auras enchant permanents (CR 303.4a)"
                            .into(),
                    )
                } else {
                    Ok(())
                }
            }
            // CR 702.6a: equip is an activated ability that only attaches to creatures you
            // control — never a spell effect, and the filter must be creature-typed.
            SpellEffectKind::Equip { target } => {
                if context == EffectContext::Spell {
                    Err("Equip is only valid on an activated ability, not a spell".into())
                } else if target.is_player() {
                    Err(format!(
                        "Equip cannot target players, got {:?}",
                        target.kind
                    ))
                } else {
                    Ok(())
                }
            }
            // CR 701.15: Regenerate puts a shield on the target; it is an activated ability, not
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

// ---------------------------------------------------------------------------
// Activated abilities
// ---------------------------------------------------------------------------

/// Cost to activate an activated ability (CR 602). Shared by every activated ability,
/// including mana abilities: an ability is classified as a mana ability (CR 605.1a) by its
/// *effect* being [`SpellEffectKind::ProduceMana`], not by its cost — so a `{T}` land, a
/// `{1}, {T}` filter land, and a sacrifice-for-mana rock all use these same cost kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilityCost {
    /// {T}: tap the source permanent.
    Tap,
    /// Pay mana (e.g. `"{4}"`, `"{2}{R}"`). Same brace syntax as `CardDefinition.mana_cost`.
    Mana(ManaCost),
    /// {T} plus mana (e.g. Jayemdae Tome: `"{4}"` + tap).
    TapAndMana(ManaCost),
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
    /// Whenever a permanent enters the battlefield (CR 603.6). The ETB-watcher analog of
    /// [`Self::WheneverPlayerCastsSpell`]: parameters control whose permanents and which type
    /// qualify. Covers Soul Warden (`controller: AnyPlayer`, `Creature`, `exclude_self`),
    /// landfall (`Controller`, `Land`), and constellation (`Controller`, `Enchantment`).
    WheneverPermanentEntersBattlefield {
        /// Whose permanents trigger this, relative to the source's controller. Defaults to
        /// `AnyPlayer` (the Soul Warden "whenever a creature enters" reading).
        #[serde(default = "any_player_trigger")]
        controller: CastTriggerPlayer,
        /// If `Some`, only permanents of this type fire the trigger. `None` matches any permanent.
        #[serde(default)]
        permanent_type: Option<PermanentTypeFilter>,
        /// If true, the source permanent's own entry does not trigger it (the "another" clause,
        /// e.g. Soul Warden). If false, the source can trigger off itself entering.
        #[serde(default)]
        exclude_self: bool,
    },
}

fn any_player_trigger() -> CastTriggerPlayer {
    CastTriggerPlayer::AnyPlayer
}

/// Serde default for `CopyTargetSpell.count` — the overwhelmingly common "make one copy".
fn one() -> u32 {
    1
}

/// Permanent card-type filter for [`TriggerCondition::WheneverPermanentEntersBattlefield`].
/// Only types that can exist on the battlefield (CR 110.4) — instants/sorceries are excluded
/// by construction, unlike [`SpellTypeFilter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermanentTypeFilter {
    Creature,
    Artifact,
    Enchantment,
    Land,
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

/// One triggered ability on a permanent (RON data tier). The effect is a plain
/// [`SpellEffectKind`] — the same effect type spells and activated abilities use. A
/// self-referencing effect (e.g. an upkeep self-pump) uses a `Self_` target filter rather
/// than a dedicated variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggeredAbilityDef {
    pub trigger: TriggerCondition,
    pub effect: SpellEffectKind,
    /// Oracle-style ability text shown as annotation on the stack card.
    pub text: String,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContinuousEffectKind {
    /// CR 613 layer 7c — modifying effects (+N/+N, -N/-N).
    PtModify {
        delta_power: i32,
        delta_toughness: i32,
    },
    /// CR 613 layer 6 — grant a keyword ability to affected permanents. Covers lords
    /// (Goblin Chieftain → Haste), pump sorceries (Overrun → Trample), and any
    /// "creatures you control gain [keyword] until end of turn" effect.
    Layer6AddKeyword(Keyword),
    // Future: Layer7bSetPt { power: i32, toughness: i32 }, …
}

// ---------------------------------------------------------------------------
// Static abilities (CR 604) and anthem/lord scopes
// ---------------------------------------------------------------------------

/// Controller restriction for an [`AnthemFilter`]. `None` on the field means "every creature in
/// play" (Crusade, Bad Moon — symmetrical anthems); `Some(YouControl)` means only the source's
/// controller's creatures (Glorious Anthem, Goblin King). An opponents-only variant is added with
/// its first card (e.g. an "opponents' creatures get -1/-1" enchantment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnthemController {
    /// Only creatures controlled by the anthem source's controller ("creatures you control").
    YouControl,
}

/// Which creatures a static anthem or one-shot mass pump applies to (CR 613). AND-combined
/// optional constraints over the creatures in play, mirroring how [`TargetFilter`] narrows a
/// chosen target. "Name two" per field: `controller` (Glorious Anthem, Goblin King) · `subtype`
/// (Lord of Atlantis = Merfolk, Goblin Chieftain = Goblin) · `color` (Crusade = White, Bad Moon =
/// Black) · `exclude_self` (every "Other ... creatures" lord). Reused by both
/// [`StaticAbilityDef::AnthemPt`] and [`SpellEffectKind::PumpAll`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AnthemFilter {
    /// `None` = every creature in play; `Some(YouControl)` = only the source controller's creatures.
    #[serde(default)]
    pub controller: Option<AnthemController>,
    /// If `Some`, only creatures whose type line contains this subtype (e.g. "Merfolk", "Goblin").
    #[serde(default)]
    pub subtype: Option<String>,
    /// If `Some`, only creatures of this color (Crusade = White, Bad Moon = Black).
    #[serde(default)]
    pub color: Option<Color>,
    /// CR "other ... creatures": exclude the anthem's own source permanent (a lord that doesn't
    /// pump itself). Ignored by [`SpellEffectKind::PumpAll`], which has no persistent source.
    #[serde(default)]
    pub exclude_self: bool,
}

/// One static ability on a permanent (CR 604) — a continuous effect that exists only while the
/// permanent is on the battlefield. Distinct from triggered/activated abilities (which use the
/// stack); the engine emits the corresponding continuous effect on ETB and drains it at LTB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaticAbilityDef {
    /// CR 613.4 layer 7c: every creature matching `filter` gets +`delta_power`/+`delta_toughness`
    /// (negative values for a debuff anthem). Anthems (Glorious Anthem) and lords (Crusade, Bad Moon).
    AnthemPt {
        #[serde(default)]
        filter: AnthemFilter,
        delta_power: i32,
        delta_toughness: i32,
    },
    /// CR 613.4 layer 7c + CR 303.4: the enchanted creature (stored as `attached_to` on the aura's
    /// `GameObject`) gets +`delta_power`/+`delta_toughness` as long as the aura remains attached.
    /// The effect drains via `WhileSourceOnBattlefield` (source = the aura permanent); it is
    /// scoped to a single permanent (`AffectedScope::Single`) so it disappears the moment the aura
    /// leaves. Holy Strength (+1/+2), Unholy Strength (+2/+1).
    AuraPtModify {
        delta_power: i32,
        delta_toughness: i32,
    },
    /// CR 301.5b / 702.6: while this equipment is attached to a creature (i.e.
    /// `attached_to` is `Some`), that creature gets +`delta_power`/+`delta_toughness`
    /// (layer 7c). The scope is `AffectedScope::EquippedBy(equipment_oid)` — it reads
    /// `attached_to` dynamically at P/T query time, so re-equipping shifts the bonus
    /// without recreating the continuous effect. Covers Bonesplitter (+2/+0) and
    /// Vulshok Morningstar (+2/+2); any equipment with a stat boost uses this variant.
    EquippedBonus {
        delta_power: i32,
        delta_toughness: i32,
    },
    /// CR 613 layer 6: every creature matching `filter` gains `keyword` while the source is on the
    /// battlefield. Covers lords (Goblin Chieftain, Captain of the Watch) and keyword-granting
    /// enchantments. Pairs with `AnthemPt` on the same card for combined "+1/+1 and haste" effects.
    AnthemKeyword {
        #[serde(default)]
        filter: AnthemFilter,
        keyword: Keyword,
    },
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
                ..Default::default()
            },
        }
        .validate(EffectContext::Spell)
        .is_ok());
    }

    #[test]
    fn player_effect_rejects_nonplayer_spec() {
        assert!(SpellEffectKind::TargetPlayerGainsLife {
            amount: 3,
            target: TargetFilter {
                kind: TargetKind::Creature,
                ..Default::default()
            },
        }
        .validate(EffectContext::Spell)
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
                amount: Amount::Fixed(3),
                target: TargetFilter {
                    kind,
                    ..Default::default()
                },
            }
            .validate(EffectContext::Spell)
            .is_ok());
        }
    }

    #[test]
    fn self_target_rejected_in_spell_context_allowed_in_ability() {
        let pump_self = SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 1,
            target: TargetFilter {
                kind: TargetKind::Self_,
                ..Default::default()
            },
        };
        assert!(pump_self.validate(EffectContext::Spell).is_err());
        assert!(pump_self.validate(EffectContext::Ability).is_ok());
    }
}
