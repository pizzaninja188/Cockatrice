use crate::mana::ManaCost;
use crate::primitives::{
    ActivatedAbilityDef, Color, Keyword, SpellEffectKind, TriggeredAbilityDef,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardDefinition {
    pub id: String,
    pub name: String,
    /// Scryfall brace syntax, copied verbatim (e.g. `"{1}{R}"`, `""` for lands). See [`ManaCost`].
    #[serde(default)]
    pub mana_cost: ManaCost,
    /// Card types followed by subtypes (e.g. `["Creature", "Bear"]`). Single source of
    /// truth for the type flags below — see [`Self::derive_type_flags`].
    #[serde(default)]
    pub types: Vec<String>,
    /// Supertypes (e.g. `["Legendary"]`). Source of truth for [`Self::is_legendary`].
    #[serde(default)]
    pub supertypes: Vec<String>,
    // Type/supertype flags: derived from `types`/`supertypes` at registry load
    // ([`Self::derive_type_flags`]), never authored in RON (`#[serde(skip)]`). They are
    // a denormalized convenience the engine reads directly; the strings are authoritative.
    #[serde(skip)]
    pub is_land: bool,
    #[serde(skip)]
    pub is_creature: bool,
    #[serde(skip)]
    pub is_instant: bool,
    #[serde(skip)]
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
    #[serde(skip)]
    pub is_artifact: bool,
    /// True for enchantment cards (used to fire enchantment-cast triggers).
    #[serde(skip)]
    pub is_enchantment: bool,
    /// Legendary supertype (for SBA: legend rule)
    #[serde(skip)]
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
    /// Populate the `#[serde(skip)]` type/supertype flags from `types`/`supertypes`,
    /// the single source of truth. Called once per card at registry load; these flags
    /// are never authored in RON. Idempotent.
    pub(crate) fn derive_type_flags(&mut self) {
        fn has(list: &[String], t: &str) -> bool {
            list.iter().any(|x| x == t)
        }
        self.is_creature = has(&self.types, "Creature");
        self.is_instant = has(&self.types, "Instant");
        self.is_sorcery = has(&self.types, "Sorcery");
        self.is_artifact = has(&self.types, "Artifact");
        self.is_enchantment = has(&self.types, "Enchantment");
        self.is_land = has(&self.types, "Land");
        self.is_legendary = has(&self.supertypes, "Legendary");
    }

    /// True for permanent cards (CR 110.4): the spell resolves to the battlefield rather
    /// than the graveyard. With validated data this is exactly "not instant, not sorcery"
    /// (land/creature/artifact/enchantment are the only other types in the schema).
    pub fn is_permanent(&self) -> bool {
        !self.is_instant && !self.is_sorcery
    }

    /// Derive the card's colors from its mana cost (CR 202.2a).
    /// A card is colorless if its mana cost contains no color symbols (e.g. lands, `{0}`, generic-only).
    pub fn colors(&self) -> Vec<Color> {
        self.mana_cost.colors()
    }
}
