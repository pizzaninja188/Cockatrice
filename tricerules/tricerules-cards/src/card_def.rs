use crate::mana::ManaCost;
use crate::primitives::{
    ActivatedAbilityDef, Color, Keyword, PermanentTypeFilter, SpellEffectKind, StaticAbilityDef,
    TriggeredAbilityDef,
};
use serde::{Deserialize, Serialize};

/// Physical card layout (CR 709/710/712/715). Drives how many faces a card has and how each
/// face becomes castable. `Normal` is the overwhelming majority — one face, the flat fields on
/// [`CardDefinition`] *are* that face. The multi-face variants populate [`CardDefinition::faces`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Layout {
    /// One face; characteristics live in the flat [`CardDefinition`] fields.
    #[default]
    Normal,
    /// CR 709: two halves printed side by side, each independently castable (Fire // Ice).
    Split,
    /// CR 712 modal DFC: either face castable from hand; the back is a real card.
    ModalDfc,
    /// CR 712 transforming DFC: front cast, transforms in place to the back (werewolves, Delver).
    Transform,
    /// CR 715: cast the adventure (spell) half, then later the creature half from exile.
    Adventure,
    /// CR 710: one card, two states stacked on one face (older Kamigawa flip cards).
    Flip,
}

/// One face of a card (CR 712.4: a card has the characteristics of its current face only).
/// Mirrors the per-face fields of [`CardDefinition`]; the whole-card fields (`id`, `layout`,
/// `partial`, `colors_override`) stay on [`CardDefinition`]. Authored only for multi-face
/// layouts — a `Normal` card's single face is the flat [`CardDefinition`] fields, not a `CardFace`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CardFace {
    pub name: String,
    /// Scryfall brace syntax, copied verbatim from this face's `card_faces[i].mana_cost`.
    #[serde(default)]
    pub mana_cost: ManaCost,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub supertypes: Vec<String>,
    #[serde(default)]
    pub power: Option<u32>,
    #[serde(default)]
    pub toughness: Option<u32>,
    #[serde(default)]
    pub spell_effect: Vec<SpellEffectKind>,
    /// Tier-3 escape hatch (CR-faithful per-card algorithm). `Some(key)` routes this face's
    /// resolution to a `CardEffect` in `tricerules-core`'s `custom` module instead of the
    /// data-driven `spell_effect` list. Mutually exclusive with a non-empty `spell_effect`
    /// (one resolution owner) — enforced at registry load. See [`CardDefinition::custom_effect`].
    #[serde(default)]
    pub custom_effect: Option<String>,
    #[serde(default)]
    pub keywords: Vec<Keyword>,
    #[serde(default)]
    pub activated_abilities: Vec<ActivatedAbilityDef>,
    #[serde(default)]
    pub triggered_abilities: Vec<TriggeredAbilityDef>,
    /// Static abilities (CR 604): anthems/lords. Omit or leave empty for cards with none.
    #[serde(default)]
    pub static_abilities: Vec<StaticAbilityDef>,
    /// CR 508.1d: "attacks each combat if able". See [`CardDefinition::must_attack_if_able`].
    #[serde(default)]
    pub must_attack_if_able: bool,
    /// CR 509.1c: "blocks each combat if able". See [`CardDefinition::must_block_if_able`].
    #[serde(default)]
    pub must_block_if_able: bool,
    // Derived type/supertype flags — same convention as CardDefinition (never authored in RON).
    #[serde(skip)]
    pub is_land: bool,
    #[serde(skip)]
    pub is_creature: bool,
    #[serde(skip)]
    pub is_instant: bool,
    #[serde(skip)]
    pub is_sorcery: bool,
    #[serde(skip)]
    pub is_artifact: bool,
    #[serde(skip)]
    pub is_enchantment: bool,
    #[serde(skip)]
    pub is_legendary: bool,
    /// CR 303.4: true when "Aura" appears in the subtype list. Auras must enchant a permanent when
    /// they enter the battlefield; they die if their enchanted permanent leaves (SBA CR 704.5m).
    #[serde(skip)]
    pub is_aura: bool,
}

impl CardFace {
    fn derive_type_flags(&mut self) {
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
        self.is_aura = has(&self.types, "Aura");
    }
}

/// A read-only view of one face's characteristics, uniform across single-face (flat fields) and
/// multi-face (a [`CardFace`]) cards. Returned by [`CardDefinition::face`] so the engine's cast
/// path and characteristic queries read "the face that is up" without caring which layout it is.
#[derive(Debug, Clone, Copy)]
pub struct FaceRef<'a> {
    pub name: &'a str,
    pub mana_cost: &'a ManaCost,
    pub types: &'a [String],
    pub supertypes: &'a [String],
    pub power: Option<u32>,
    pub toughness: Option<u32>,
    pub spell_effect: &'a [SpellEffectKind],
    /// Tier-3 custom resolution key for this face, if any (see [`CardFace::custom_effect`]).
    pub custom_effect: Option<&'a str>,
    pub keywords: &'a [Keyword],
    pub activated_abilities: &'a [ActivatedAbilityDef],
    pub triggered_abilities: &'a [TriggeredAbilityDef],
    pub static_abilities: &'a [StaticAbilityDef],
    pub is_land: bool,
    pub is_creature: bool,
    pub is_instant: bool,
    pub is_sorcery: bool,
    pub is_artifact: bool,
    pub is_enchantment: bool,
    pub is_legendary: bool,
}

impl FaceRef<'_> {
    /// True for permanent faces (CR 110.4): resolves to the battlefield, not the graveyard.
    pub fn is_permanent(&self) -> bool {
        !self.is_instant && !self.is_sorcery
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CardDefinition {
    pub id: String,
    /// The whole-card Oracle name. For multi-face cards this is the `//` name
    /// (e.g. `"Fire // Ice"`) that `cards.xml` stores and decks reference; the slug invariant
    /// (`id == slugify(name)`) keys off it. Per-face names live in [`CardFace::name`].
    pub name: String,
    /// Physical layout (CR 709/710/712/715). `Normal` (default) leaves [`Self::faces`] empty and
    /// the single face is the flat fields below. Multi-face layouts populate [`Self::faces`].
    #[serde(default)]
    pub layout: Layout,
    /// The card's faces, authored only for multi-face layouts (`faces.len() == 2` for split/MDFC/
    /// transform/adventure/flip). Empty for `Normal`, whose lone face *is* the flat fields below;
    /// [`Self::face`] hides this asymmetry so call sites read faces uniformly.
    #[serde(default)]
    pub faces: Vec<CardFace>,
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
    /// Tier-3 escape hatch: `Some(key)` resolves this card via a `CardEffect` (a CR-faithful
    /// per-card algorithm in `tricerules-core`'s `custom` module) rather than the data-driven
    /// `spell_effect` list — for cards whose resolution is a unique algorithm (mid-resolution
    /// player choice over live objects), not `(effect_kind, parameters)` static data. Mutually
    /// exclusive with a non-empty `spell_effect` (one resolution owner), enforced at registry load.
    #[serde(default)]
    pub custom_effect: Option<String>,
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
    /// CR 303.4: "Aura" subtype — enters attached to a permanent, dies if enchanted leaves.
    #[serde(skip)]
    pub is_aura: bool,
    /// Activated abilities (cost + effect pairs). Omit or leave empty for cards with none.
    #[serde(default)]
    pub activated_abilities: Vec<ActivatedAbilityDef>,
    /// Triggered abilities (trigger condition + effect pairs). Omit or leave empty for cards with none.
    #[serde(default)]
    pub triggered_abilities: Vec<TriggeredAbilityDef>,
    /// Static abilities (CR 604): anthems and lords (Glorious Anthem, Crusade, Bad Moon). Omit or
    /// leave empty for cards with none. Emitted as a continuous effect on ETB, drained at LTB.
    #[serde(default)]
    pub static_abilities: Vec<StaticAbilityDef>,
    /// CR 508.1d: "attacks each combat if able". This creature must be declared as an attacker
    /// whenever it is a legal attacker. Cards: Crazed Goblin, Goblin Brigand, Juggernaut.
    #[serde(default)]
    pub must_attack_if_able: bool,
    /// CR 509.1c: "blocks each combat if able". This creature must be declared as a blocker
    /// whenever it could legally block an attacking creature.
    #[serde(default)]
    pub must_block_if_able: bool,
    /// Implementation tracking only (ignored by the engine):
    /// `Some("what's missing")` = partially implemented; `None` = fully implemented.
    #[serde(default)]
    pub partial: Option<String>,
    /// Explicit colors, set only when this definition is synthesized from a
    /// [`TokenDefinition`](crate::token_def::TokenDefinition) (CR 111.4: a token's color comes
    /// from the creating effect, not a mana cost). `None` for normal cards, whose colors are
    /// derived from `mana_cost` — see [`Self::colors`]. Never authored in card RON.
    #[serde(skip)]
    pub colors_override: Option<Vec<Color>>,
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
        self.is_aura = has(&self.types, "Aura");
        for face in &mut self.faces {
            face.derive_type_flags();
        }
    }

    /// Number of faces (CR 712.4). `1` for a `Normal` card (the flat fields), else `faces.len()`.
    pub fn face_count(&self) -> usize {
        if self.faces.is_empty() {
            1
        } else {
            self.faces.len()
        }
    }

    /// A uniform view of face `index`, or `None` if out of range. Index `0` of a `Normal` card is
    /// the flat fields; multi-face cards index into [`Self::faces`]. The engine casts and reads
    /// characteristics through this so split/MDFC/transform need no per-call-site branching.
    pub fn face(&self, index: usize) -> Option<FaceRef<'_>> {
        if self.faces.is_empty() {
            return if index == 0 {
                Some(FaceRef {
                    name: &self.name,
                    mana_cost: &self.mana_cost,
                    types: &self.types,
                    supertypes: &self.supertypes,
                    power: self.power,
                    toughness: self.toughness,
                    spell_effect: &self.spell_effect,
                    custom_effect: self.custom_effect.as_deref(),
                    keywords: &self.keywords,
                    activated_abilities: &self.activated_abilities,
                    triggered_abilities: &self.triggered_abilities,
                    static_abilities: &self.static_abilities,
                    is_land: self.is_land,
                    is_creature: self.is_creature,
                    is_instant: self.is_instant,
                    is_sorcery: self.is_sorcery,
                    is_artifact: self.is_artifact,
                    is_enchantment: self.is_enchantment,
                    is_legendary: self.is_legendary,
                })
            } else {
                None
            };
        }
        self.faces.get(index).map(|f| FaceRef {
            name: &f.name,
            mana_cost: &f.mana_cost,
            types: &f.types,
            supertypes: &f.supertypes,
            power: f.power,
            toughness: f.toughness,
            spell_effect: &f.spell_effect,
            custom_effect: f.custom_effect.as_deref(),
            keywords: &f.keywords,
            activated_abilities: &f.activated_abilities,
            triggered_abilities: &f.triggered_abilities,
            static_abilities: &f.static_abilities,
            is_land: f.is_land,
            is_creature: f.is_creature,
            is_instant: f.is_instant,
            is_sorcery: f.is_sorcery,
            is_artifact: f.is_artifact,
            is_enchantment: f.is_enchantment,
            is_legendary: f.is_legendary,
        })
    }

    /// The primary face (front face / face 0). Always present — every card has at least one face.
    pub fn primary_face(&self) -> FaceRef<'_> {
        self.face(0).expect("every card has a face 0")
    }

    /// Iterate every face in index order (length [`Self::face_count`]).
    pub fn faces_iter(&self) -> impl Iterator<Item = FaceRef<'_>> {
        (0..self.face_count()).map(move |i| self.face(i).expect("face index in range"))
    }

    /// True if this card has more than one face (any non-`Normal` layout).
    pub fn is_multiface(&self) -> bool {
        !self.faces.is_empty()
    }

    /// True for permanent cards (CR 110.4): the spell resolves to the battlefield rather
    /// than the graveyard. With validated data this is exactly "not instant, not sorcery"
    /// (land/creature/artifact/enchantment are the only other types in the schema).
    pub fn is_permanent(&self) -> bool {
        !self.is_instant && !self.is_sorcery
    }

    /// True if this card's types satisfy a [`PermanentTypeFilter`] (used by ETB-watcher
    /// triggers like Soul Warden / landfall / constellation). Reads the derived type flags.
    pub fn is_permanent_type(&self, filter: PermanentTypeFilter) -> bool {
        match filter {
            PermanentTypeFilter::Creature => self.is_creature,
            PermanentTypeFilter::Artifact => self.is_artifact,
            PermanentTypeFilter::Enchantment => self.is_enchantment,
            PermanentTypeFilter::Land => self.is_land,
        }
    }

    /// Derive the card's colors from its mana cost (CR 202.2a), or use the explicit
    /// [`colors_override`](Self::colors_override) for token definitions (CR 111.4).
    /// A card is colorless if its mana cost contains no color symbols (e.g. lands, `{0}`, generic-only).
    pub fn colors(&self) -> Vec<Color> {
        if let Some(colors) = &self.colors_override {
            return colors.clone();
        }
        self.mana_cost.colors()
    }
}
