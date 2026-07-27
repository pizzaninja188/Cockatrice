//! Colors and parameterless keyword abilities used by card characteristics.

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
