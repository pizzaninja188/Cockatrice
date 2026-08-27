//! Colors, parameterless keywords, and parameterized evasion abilities used by card
//! characteristics.

use super::PowerComparison;
use serde::{Deserialize, Serialize};

/// The five MTG colors. Used for characteristic-based blocking checks (Intimidate, Protection)
/// and derived from a card's mana cost at query time — not stored as a separate RON field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Color {
    White,
    Blue,
    Black,
    Red,
    Green,
}

/// A parameterized evasion ability, kept separate from [`Keyword`] so one data-tier value can
/// represent every matching land subtype rather than adding a keyword variant per subtype.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Evasion {
    /// CR 702.14c: this creature can't be blocked while the defending player controls a land
    /// with `land_subtype` (River Boa's Islandwalk, Shanodin Dryads' Forestwalk).
    Landwalk { land_subtype: String },
    /// CR 509.1b: creatures whose current derived power matches `comparison` cannot block this
    /// attacker. Foggy Swamp Vinebender and Arlinn's Wolf use `AtMost(2)`.
    BlockerPower { comparison: PowerComparison },
    /// CR 509.1b: a completed declaration may assign at most this many blockers to this attacker.
    /// Safewright Cavalry and Bristling Boar use one; zero is the ordinary unblockable shape.
    BlockerCountMaximum { maximum: u32 },
}

/// Exact card types currently represented by the ordinary ruled-game card model. This is kept
/// separate from `CardTypeFilter`: protection names a characteristic value, not a predicate such
/// as "noncreature" or "instant or sorcery" (CR 702.16a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtectionCardType {
    Artifact,
    Creature,
    Enchantment,
    Instant,
    Kindred,
    Land,
    Planeswalker,
    Sorcery,
}

impl ProtectionCardType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Artifact => "Artifact",
            Self::Creature => "Creature",
            Self::Enchantment => "Enchantment",
            Self::Instant => "Instant",
            Self::Kindred => "Kindred",
            Self::Land => "Land",
            Self::Planeswalker => "Planeswalker",
            Self::Sorcery => "Sorcery",
        }
    }

    fn protection_label(self) -> &'static str {
        match self {
            Self::Artifact => "artifacts",
            Self::Creature => "creatures",
            Self::Enchantment => "enchantments",
            Self::Instant => "instants",
            Self::Kindred => "Kindred",
            Self::Land => "lands",
            Self::Planeswalker => "planeswalkers",
            Self::Sorcery => "sorceries",
        }
    }
}

/// The quality named by one instance of protection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtectionQuality {
    Color(Color),
    CardType(ProtectionCardType),
}

impl ProtectionQuality {
    pub fn label(self) -> String {
        format!("Protection from {}", self.quality_label())
    }

    pub fn choice_label(self) -> &'static str {
        match self {
            Self::Color(Color::White) => "White",
            Self::Color(Color::Blue) => "Blue",
            Self::Color(Color::Black) => "Black",
            Self::Color(Color::Red) => "Red",
            Self::Color(Color::Green) => "Green",
            Self::CardType(card_type) => card_type.protection_label(),
        }
    }

    fn quality_label(self) -> &'static str {
        match self {
            Self::Color(Color::White) => "white",
            Self::Color(Color::Blue) => "blue",
            Self::Color(Color::Black) => "black",
            Self::Color(Color::Red) => "red",
            Self::Color(Color::Green) => "green",
            Self::CardType(card_type) => card_type.protection_label(),
        }
    }

    pub fn matches(self, colors: &[Color], types: &[String]) -> bool {
        match self {
            Self::Color(color) => colors.contains(&color),
            Self::CardType(card_type) => types.iter().any(|value| value == card_type.as_str()),
        }
    }
}

/// Static keyword abilities that affect game rules (blocking restrictions, attack rules, damage
/// modifiers, etc.). Parameterless only; parameterized values live in dedicated data-tier types
/// such as [`Evasion`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Keyword {
    /// CR 702.51: Unexpected Assistance and Merrow Skyswimmer.
    Convoke,
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
    /// Whether this keyword may be represented by a keyword counter under CR 122.1b.
    ///
    /// Keep this list narrower than the engine's general keyword vocabulary: defender, flash,
    /// intimidate, and shroud are rules keywords, but they are not keyword-counter kinds.
    pub fn can_be_keyword_counter(self) -> bool {
        matches!(
            self,
            Keyword::Flying
                | Keyword::Reach
                | Keyword::Vigilance
                | Keyword::Lifelink
                | Keyword::Haste
                | Keyword::Deathtouch
                | Keyword::Menace
                | Keyword::Trample
                | Keyword::FirstStrike
                | Keyword::DoubleStrike
                | Keyword::Indestructible
                | Keyword::Hexproof
        )
    }

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
            Keyword::Convoke => "Convoke",
        }
    }
}
