use crate::primitives::{ActivatedAbilityDef, Color, Keyword, SpellEffectKind, TriggeredAbilityDef};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardDefinition {
    pub id: String,
    pub name: String,
    /// e.g. "R" or "1R" — minimal parser in engine
    pub mana_cost: String,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub supertypes: Vec<String>,
    #[serde(default)]
    pub is_land: bool,
    #[serde(default)]
    pub is_creature: bool,
    #[serde(default)]
    pub is_instant: bool,
    #[serde(default)]
    pub is_sorcery: bool,
    #[serde(default)]
    pub power: Option<u32>,
    #[serde(default)]
    pub toughness: Option<u32>,
    /// Data-driven spell effects resolved in order from the same target list
    /// (see [`SpellEffectKind`]). RON: `spell_effect: [DamageTarget(...), Draw(count: 1)]`.
    #[serde(default)]
    pub spell_effect: Vec<SpellEffectKind>,
    /// Static keyword abilities (Flying, Reach, Intimidate, etc.). Omit or leave empty for keywordless cards.
    #[serde(default)]
    pub keywords: Vec<Keyword>,
    /// True for artifact cards (Artifact, Artifact Creature, etc.).
    /// Artifact creatures can block creatures with Intimidate regardless of color.
    #[serde(default)]
    pub is_artifact: bool,
    /// True for enchantment cards (used to fire enchantment-cast triggers).
    #[serde(default)]
    pub is_enchantment: bool,
    /// Legendary supertype (for SBA: legend rule)
    #[serde(default)]
    pub is_legendary: bool,
    /// Activated abilities (cost + effect pairs). Omit or leave empty for cards with none.
    #[serde(default)]
    pub activated_abilities: Vec<ActivatedAbilityDef>,
    /// Triggered abilities (trigger condition + effect pairs). Omit or leave empty for cards with none.
    #[serde(default)]
    pub triggered_abilities: Vec<TriggeredAbilityDef>,
    /// Implementation tracking only (ignored by the engine):
    /// `Some("what's missing")` = partially implemented; `None` = fully implemented.
    #[serde(default)]
    pub partial: Option<String>,
}

impl CardDefinition {
    /// Derive the card's colors from its mana cost (CR 202.2a).
    /// A card is colorless if its mana cost contains no color symbols (e.g. lands, "0", generic-only).
    pub fn colors(&self) -> Vec<Color> {
        let mut out = Vec::new();
        for ch in self.mana_cost.chars() {
            let c = match ch {
                'W' => Color::White,
                'U' => Color::Blue,
                'B' => Color::Black,
                'R' => Color::Red,
                'G' => Color::Green,
                _ => continue,
            };
            if !out.contains(&c) {
                out.push(c);
            }
        }
        out
    }
}
