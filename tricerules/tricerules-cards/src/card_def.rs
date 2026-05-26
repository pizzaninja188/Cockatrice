use crate::primitives::{Keyword, SpellEffectKind};
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
    /// Data-driven spell effect (see [`SpellEffectKind`]); deserialized
    /// directly from RON, e.g. `DamageTarget(amount: 3, target: AnyTarget)`.
    #[serde(default)]
    pub spell_effect: Option<SpellEffectKind>,
    /// Static keyword abilities (Flying, Reach, etc.). Omit or leave empty for keywordless cards.
    #[serde(default)]
    pub keywords: Vec<Keyword>,
    /// Legendary supertype (for SBA: legend rule)
    #[serde(default)]
    pub is_legendary: bool,
    /// Implementation tracking only (ignored by the engine):
    /// `Some("what's missing")` = partially implemented; `None` = fully implemented.
    #[serde(default)]
    pub partial: Option<String>,
}
