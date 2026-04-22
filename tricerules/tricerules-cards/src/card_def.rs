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
    /// Effect key for data-driven spells (see primitives)
    #[serde(default)]
    pub spell_effect: Option<String>,
}
