//! Token definitions — the registry sub-namespace for objects created by effects (CR 111).
//!
//! A token has no Oracle card and no deck entry: its identity is minted at resolution by a
//! [`CreateTokens`](crate::primitives::SpellEffectKind::CreateTokens) effect. Tokens live in their
//! own RON namespace under `data/tokens/` and are loaded separately from deck cards, but the engine
//! must query a token's characteristics (types, P/T, colors, keywords) through the *same* path it
//! uses for cards. So at load each [`TokenDefinition`] is converted into a [`CardDefinition`]
//! (with `mana_cost` empty and `colors_override` carrying its printed colors) and stored in the
//! registry's token map — `CardRegistry::get` falls back to it, so the engine never branches on
//! token-ness for a characteristic lookup.

use crate::card_def::CardDefinition;
use crate::primitives::{Color, Keyword};
use serde::{Deserialize, Serialize};

/// A token's printed characteristics, authored in `data/tokens/*.ron`. Mirrors the subset of
/// [`CardDefinition`] that a token can have — no mana cost, no spell effect (tokens are never cast).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDefinition {
    /// Token id; by convention the slug of `name` (enforced by a registry test, like cards).
    /// Referenced by [`CreateTokens.token`](crate::primitives::SpellEffectKind::CreateTokens).
    pub id: String,
    /// Display name, e.g. `"Soldier"`. Tokens with the same name but different characteristics
    /// (CR 111.4) get distinct ids.
    pub name: String,
    /// Card types followed by subtypes, e.g. `["Creature", "Soldier"]`. Source of the type flags.
    #[serde(default)]
    pub types: Vec<String>,
    /// Supertypes (rare on tokens; e.g. Legendary for some emblem-adjacent tokens).
    #[serde(default)]
    pub supertypes: Vec<String>,
    /// Colors granted by the creating effect (CR 111.4) — *not* derived from a mana cost.
    /// Empty = colorless.
    #[serde(default)]
    pub colors: Vec<Color>,
    #[serde(default)]
    pub power: Option<u32>,
    #[serde(default)]
    pub toughness: Option<u32>,
    /// Static keyword abilities printed on the token (e.g. Flying for a Spirit token).
    #[serde(default)]
    pub keywords: Vec<Keyword>,
}

impl TokenDefinition {
    /// Project this token into a [`CardDefinition`] so the engine's characteristic queries
    /// (`is_creature`, P/T base, `keywords`, `colors`) work uniformly. The type flags are
    /// derived by the caller via [`CardDefinition::derive_type_flags`] after construction.
    pub fn to_card_def(&self) -> CardDefinition {
        CardDefinition {
            id: self.id.clone(),
            name: self.name.clone(),
            types: self.types.clone(),
            supertypes: self.supertypes.clone(),
            power: self.power,
            toughness: self.toughness,
            keywords: self.keywords.clone(),
            colors_override: Some(self.colors.clone()),
            ..Default::default()
        }
    }
}
