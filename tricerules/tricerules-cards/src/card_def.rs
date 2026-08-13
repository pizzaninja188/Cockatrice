//! Card definitions — the rules-side identity of a card and its faces.
//!
//! There are two shapes here on purpose:
//!
//! * [`RawCardDefinition`] is the *authoring* schema — exactly what a `data/**/*.ron` file
//!   contains. A single-face card is authored flat (`mana_cost`, `types`, `power`, … at the top
//!   level); a multi-face card authors a `faces: [...]` list instead. 35k generated files must
//!   never be forced into a `faces:` wrapper, so this shape stays.
//! * [`CardDefinition`] is the *runtime* shape the engine reads: whole-card identity plus a
//!   non-empty `faces` vec. Registry load converts raw → runtime once, normalizing the flat
//!   single-face authoring into `faces[0]`.
//!
//! The payoff is that a new per-card characteristic is declared once, on [`CardFace`], instead of
//! three times (flat field + face field + borrowed mirror).

use crate::mana::ManaCost;
use crate::primitives::{
    ActivatedAbilityDef, AdditionalCost, CardTypeFilter, Color, Evasion, Keyword,
    PermanentTypeFilter, SpellCostModifier, SpellEffectKind, StaticAbilityDef, TargetingDef,
    TriggeredAbilityDef,
};
use serde::{Deserialize, Serialize};

/// One printed mode of a modal spell. Its effects resolve in authored order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpellModeDef {
    /// Short client-facing description of the mode, without the card name.
    pub label: String,
    /// Data-driven effects for this mode, resolved from this mode's own target group.
    #[serde(default)]
    pub effects: Vec<SpellEffectKind>,
    #[serde(default)]
    pub targeting: Option<TargetingDef>,
}

/// The choose-N definition of a modal spell (CR 700.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalSpellDef {
    pub min_modes: u32,
    pub max_modes: u32,
    /// Modes in printed order. A mode may be selected at most once in the initial implementation.
    #[serde(default)]
    pub modes: Vec<SpellModeDef>,
}

/// Physical card layout (CR 709/710/712/715). Drives how many faces a card has and how each
/// face becomes castable. `Normal` is the overwhelming majority — one face, authored flat.
/// The multi-face variants author [`RawCardDefinition::faces`] explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Layout {
    /// One face, authored with the flat [`RawCardDefinition`] fields.
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

/// One face of a card (CR 712.4: a card has the characteristics of its current face only) — the
/// single home for every per-card characteristic. The whole-card fields (`id`, `name`, `layout`,
/// `partial`) live on [`CardDefinition`].
///
/// A `Normal` card has exactly one of these, built at registry load from the flat authoring
/// fields; multi-face layouts author theirs directly.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CardFace {
    pub name: String,
    /// Scryfall brace syntax, copied verbatim from this face's `card_faces[i].mana_cost`.
    #[serde(default)]
    pub mana_cost: ManaCost,
    /// CR 702.34: optional cost to cast this face from its owner's graveyard.
    #[serde(default)]
    pub flashback_cost: Option<ManaCost>,
    /// Mandatory nonmana costs paid in addition to this face's mana cost (CR 118.8).
    #[serde(default)]
    pub additional_costs: Vec<AdditionalCost>,
    /// Face-authored adjustments used to determine this spell's total cost (CR 601.2f).
    #[serde(default)]
    pub cost_modifiers: Vec<SpellCostModifier>,
    /// Card types followed by subtypes (e.g. `["Creature", "Bear"]`). Single source of truth for
    /// the type flags below — see [`CardFace::derive_type_flags`].
    #[serde(default)]
    pub types: Vec<String>,
    /// Supertypes (e.g. `["Legendary"]`). Source of truth for [`Self::is_legendary`].
    #[serde(default)]
    pub supertypes: Vec<String>,
    #[serde(default)]
    pub power: Option<u32>,
    #[serde(default)]
    pub toughness: Option<u32>,
    /// Data-driven spell effects resolved in order from the same target list
    /// (see [`SpellEffectKind`]). RON: `spell_effect: [DamageTarget(...), Draw(count: 1)]`.
    #[serde(default)]
    pub spell_effect: Vec<SpellEffectKind>,
    #[serde(default)]
    pub targeting: Option<TargetingDef>,
    /// Modal data-driven spell effects. Mutually exclusive with `spell_effect` and
    /// `custom_effect`; selected modes resolve in printed order.
    #[serde(default)]
    pub modal_spell: Option<ModalSpellDef>,
    /// Tier-3 escape hatch (CR-faithful per-card algorithm). `Some(key)` routes this face's
    /// resolution to a `CardEffect` in `tricerules-core`'s `custom` module instead of the
    /// data-driven `spell_effect` list — for cards whose resolution is a unique algorithm
    /// (mid-resolution player choice over live objects), not `(effect_kind, parameters)` static
    /// data. Mutually exclusive with a non-empty `spell_effect` (one resolution owner per face,
    /// CR 608) — enforced at registry load.
    #[serde(default)]
    pub custom_effect: Option<String>,
    /// Static keyword abilities (Flying, Reach, Intimidate, …). Omit for keywordless faces.
    #[serde(default)]
    pub keywords: Vec<Keyword>,
    /// Parameterized combat evasion abilities (Islandwalk, Forestwalk, …).
    #[serde(default)]
    pub evasions: Vec<Evasion>,
    /// Activated abilities (cost + effect pairs). Omit or leave empty for faces with none.
    #[serde(default)]
    pub activated_abilities: Vec<ActivatedAbilityDef>,
    /// Triggered abilities (trigger condition + effect pairs). Omit or leave empty for faces
    /// with none.
    #[serde(default)]
    pub triggered_abilities: Vec<TriggeredAbilityDef>,
    /// Static abilities (CR 604): anthems and lords (Glorious Anthem, Crusade, Bad Moon). Omit or
    /// leave empty for faces with none. Emitted as a continuous effect on ETB, drained at LTB.
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
    /// CR 105.2: an authored color indicator printed to the left of the type line.
    #[serde(default)]
    pub color_indicator: Option<Vec<Color>>,
    /// Explicit colors for a face synthesized from a
    /// [`TokenDefinition`](crate::token_def::TokenDefinition) (CR 111.4: a token's color comes from
    /// the creating effect, not a mana cost). `None` for printed faces, whose colors derive from
    /// `mana_cost` — see [`Self::colors`]. Never authored in card RON.
    #[serde(skip)]
    pub colors_override: Option<Vec<Color>>,
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
    /// Populate the `#[serde(skip)]` type/supertype flags from `types`/`supertypes`, the single
    /// source of truth. Called once per face at registry load. Idempotent.
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
    }

    /// True for permanent faces (CR 110.4): resolves to the battlefield, not the graveyard.
    /// With validated data this is exactly "not instant, not sorcery" (land/creature/artifact/
    /// enchantment are the only other types in the schema).
    pub fn is_permanent(&self) -> bool {
        !self.is_instant && !self.is_sorcery
    }

    /// This face's colors, derived from its mana cost (CR 202.2a), or the explicit
    /// [`colors_override`](Self::colors_override) for token faces (CR 111.4). A face is colorless
    /// when its mana cost carries no color symbols (lands, `{0}`, generic-only costs).
    pub fn colors(&self) -> Vec<Color> {
        match &self.colors_override {
            Some(colors) => colors.clone(),
            None => self
                .color_indicator
                .clone()
                .unwrap_or_else(|| self.mana_cost.colors()),
        }
    }

    /// True if this face's types satisfy a [`PermanentTypeFilter`] (ETB-watcher triggers like
    /// Soul Warden / landfall / constellation). Reads the derived type flags.
    pub fn is_permanent_type(&self, filter: PermanentTypeFilter) -> bool {
        match filter {
            PermanentTypeFilter::Creature => self.is_creature,
            PermanentTypeFilter::Artifact => self.is_artifact,
            PermanentTypeFilter::Enchantment => self.is_enchantment,
            PermanentTypeFilter::Land => self.is_land,
        }
    }

    /// Whether this face has the card type selected by `filter`. Stack objects use the face that
    /// was cast; cards in other zones first select their applicable face set through
    /// [`CardDefinition::matches_card_type_outside_stack`].
    pub fn matches_card_type(&self, filter: CardTypeFilter) -> bool {
        match filter {
            CardTypeFilter::Enchantment => self.is_enchantment,
            CardTypeFilter::Instant => self.is_instant,
            CardTypeFilter::Sorcery => self.is_sorcery,
            CardTypeFilter::InstantOrSorcery => self.is_instant || self.is_sorcery,
            CardTypeFilter::Creature => self.is_creature,
            CardTypeFilter::Artifact => self.is_artifact,
            CardTypeFilter::Noncreature => !self.is_creature,
        }
    }
}

/// A read-only view of one face. Kept as a named alias because the engine reads faces constantly
/// ("the face that is up") and the name documents that intent at call sites.
pub type FaceRef<'a> = &'a CardFace;

/// The authored RON shape of a card, deserialized as-is and immediately converted to a
/// [`CardDefinition`] by [`Self::into_definition`]. Nothing outside registry load sees it.
///
/// Single-face cards are authored flat (no `faces:` wrapper) — this is the schema the ~870
/// hand-authored and generated files use, and it is deliberately unchanged.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawCardDefinition {
    pub id: String,
    /// The whole-card Oracle name. For multi-face cards this is the `//` name
    /// (e.g. `"Fire // Ice"`) that `cards.xml` stores and decks reference; the slug invariant
    /// (`id == slugify(name)`) keys off it. Per-face names live in [`CardFace::name`].
    pub name: String,
    /// Physical layout (CR 709/710/712/715). `Normal` (default) is authored flat; every other
    /// layout authors [`Self::faces`].
    #[serde(default)]
    pub layout: Layout,
    /// The card's faces, authored only for multi-face layouts (`faces.len() == 2` for split/MDFC/
    /// transform/adventure/flip). Empty for `Normal`, whose lone face is the flat fields below.
    #[serde(default)]
    pub faces: Vec<CardFace>,
    /// Implementation tracking only (ignored by the engine):
    /// `Some("what's missing")` = partially implemented; `None` = fully implemented.
    #[serde(default)]
    pub partial: Option<String>,
    // ---- Flat single-face authoring fields; mirror [`CardFace`] one-for-one. ----
    /// Scryfall brace syntax, copied verbatim (e.g. `"{1}{R}"`, `""` for lands). See [`ManaCost`].
    #[serde(default)]
    pub mana_cost: ManaCost,
    /// CR 702.34: optional cost to cast this face from its owner's graveyard.
    #[serde(default)]
    pub flashback_cost: Option<ManaCost>,
    #[serde(default)]
    pub additional_costs: Vec<AdditionalCost>,
    #[serde(default)]
    pub cost_modifiers: Vec<SpellCostModifier>,
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
    #[serde(default)]
    pub targeting: Option<TargetingDef>,
    #[serde(default)]
    pub modal_spell: Option<ModalSpellDef>,
    #[serde(default)]
    pub custom_effect: Option<String>,
    #[serde(default)]
    pub keywords: Vec<Keyword>,
    #[serde(default)]
    pub evasions: Vec<Evasion>,
    #[serde(default)]
    pub activated_abilities: Vec<ActivatedAbilityDef>,
    #[serde(default)]
    pub triggered_abilities: Vec<TriggeredAbilityDef>,
    #[serde(default)]
    pub static_abilities: Vec<StaticAbilityDef>,
    #[serde(default)]
    pub must_attack_if_able: bool,
    #[serde(default)]
    pub must_block_if_able: bool,
    #[serde(default)]
    pub color_indicator: Option<Vec<Color>>,
}

impl RawCardDefinition {
    /// Normalize authored RON into the runtime shape: a `Normal` card's flat fields become
    /// `faces[0]`, a multi-face card keeps its authored faces. Returns the reason string for an
    /// authoring mistake the schema itself cannot express (registry load turns it into
    /// `RegistryError::InvalidCard`).
    pub(crate) fn into_definition(self) -> Result<CardDefinition, String> {
        let multiface_layout = self.layout != Layout::Normal;
        if multiface_layout && self.faces.is_empty() {
            return Err(format!(
                "layout {:?} requires an authored `faces` list",
                self.layout
            ));
        }
        if !multiface_layout && !self.faces.is_empty() {
            return Err("`faces` authored on a Normal-layout card (set an explicit layout)".into());
        }
        let faces = if self.faces.is_empty() {
            vec![CardFace {
                name: self.name.clone(),
                mana_cost: self.mana_cost,
                flashback_cost: self.flashback_cost,
                additional_costs: self.additional_costs,
                cost_modifiers: self.cost_modifiers,
                types: self.types,
                supertypes: self.supertypes,
                power: self.power,
                toughness: self.toughness,
                spell_effect: self.spell_effect,
                targeting: self.targeting,
                modal_spell: self.modal_spell,
                custom_effect: self.custom_effect,
                keywords: self.keywords,
                evasions: self.evasions,
                activated_abilities: self.activated_abilities,
                triggered_abilities: self.triggered_abilities,
                static_abilities: self.static_abilities,
                must_attack_if_able: self.must_attack_if_able,
                must_block_if_able: self.must_block_if_able,
                color_indicator: self.color_indicator,
                ..Default::default()
            }]
        } else {
            self.faces
        };
        Ok(CardDefinition {
            id: self.id,
            name: self.name,
            layout: self.layout,
            faces,
            partial: self.partial,
        })
    }
}

/// The runtime rules identity of a card: whole-card fields plus its faces (CR 712.4). Every
/// characteristic — cost, types, P/T, effects, abilities — is read from a face, never from here.
#[derive(Debug, Clone, Default)]
pub struct CardDefinition {
    pub id: String,
    /// The whole-card Oracle name. For multi-face cards this is the `//` name
    /// (e.g. `"Fire // Ice"`) that `cards.xml` stores and decks reference; the slug invariant
    /// (`id == slugify(name)`) keys off it. Per-face names live in [`CardFace::name`].
    pub name: String,
    /// Physical layout (CR 709/710/712/715).
    pub layout: Layout,
    /// The card's faces in printed order, always non-empty: one for `Normal`, two for
    /// split/MDFC/transform/adventure/flip.
    pub faces: Vec<CardFace>,
    /// Implementation tracking only (ignored by the engine):
    /// `Some("what's missing")` = partially implemented; `None` = fully implemented.
    pub partial: Option<String>,
}

impl CardDefinition {
    /// Display-database identity for `face_index`. Cockatrice stores transform, flip, and MDFC
    /// faces as separate entries, while split and Adventure cards retain the whole-card entry.
    pub fn face_display_name(&self, face_index: usize) -> Option<&str> {
        let face = self.face(face_index)?;
        Some(match self.layout {
            Layout::Transform | Layout::Flip | Layout::ModalDfc => face.name.as_str(),
            Layout::Normal | Layout::Split | Layout::Adventure => self.name.as_str(),
        })
    }

    /// Populate every face's derived type/supertype flags. Called once per card at registry
    /// load; the flags are never authored in RON. Idempotent.
    pub(crate) fn derive_type_flags(&mut self) {
        for face in &mut self.faces {
            face.derive_type_flags();
        }
    }

    /// Number of faces (CR 712.4). `1` for a `Normal` card.
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// A view of face `index`, or `None` if out of range. The engine casts and reads
    /// characteristics through this so split/MDFC/transform need no per-call-site branching.
    pub fn face(&self, index: usize) -> Option<FaceRef<'_>> {
        self.faces.get(index)
    }

    /// The primary face (front face / face 0). Always present — every card has at least one face.
    pub fn primary_face(&self) -> FaceRef<'_> {
        self.faces.first().expect("every card has a face 0")
    }

    /// Iterate every face in printed order (length [`Self::face_count`]).
    pub fn faces_iter(&self) -> impl Iterator<Item = FaceRef<'_>> {
        self.faces.iter()
    }

    /// True if this card has more than one face (any non-`Normal` layout).
    pub fn is_multiface(&self) -> bool {
        self.faces.len() > 1
    }

    /// Whether this physical card matches `filter` in a zone other than the battlefield or stack.
    /// Split cards combine both halves (CR 709.4). Flip, double-faced, and adventurer cards use
    /// only their normal/front characteristics there (CR 710.2, 712.8a, 715.4).
    pub fn matches_card_type_outside_stack(&self, filter: CardTypeFilter) -> bool {
        if self.layout != Layout::Split {
            return self.primary_face().matches_card_type(filter);
        }

        match filter {
            CardTypeFilter::Noncreature => self.faces_iter().all(|face| !face.is_creature),
            _ => self.faces_iter().any(|face| face.matches_card_type(filter)),
        }
    }

    /// Whether this physical card has `name` in a zone other than the battlefield or stack.
    /// Split cards have both half names there (CR 709.4); flip, double-faced, and adventurer cards
    /// use only their normal/front face (CR 710.2, 712.8a, 715.4).
    pub fn has_name_outside_stack(&self, name: &str) -> bool {
        if self.layout == Layout::Split {
            self.faces_iter().any(|face| face.name == name)
        } else {
            self.primary_face().name == name
        }
    }

    /// Whether `face_index` is a face the player may choose while playing this card from hand.
    /// Split cards, modal DFCs, and Adventures expose both authored spell/land choices there;
    /// transforming DFCs and flip cards expose only their front/top face. This is a layout rule,
    /// independent of timing, costs, targets, and whether the chosen face is a spell or land.
    pub fn face_available_from_hand(&self, face_index: usize) -> bool {
        match self.layout {
            Layout::Normal | Layout::Transform | Layout::Flip => face_index == 0,
            Layout::Split | Layout::ModalDfc | Layout::Adventure => face_index < self.face_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(types: &[&str]) -> CardFace {
        let mut face = CardFace {
            types: types.iter().map(|value| (*value).to_owned()).collect(),
            ..CardFace::default()
        };
        face.derive_type_flags();
        face
    }

    fn definition(layout: Layout, faces: Vec<CardFace>) -> CardDefinition {
        CardDefinition {
            id: "test_card".to_owned(),
            name: "Test Card".to_owned(),
            layout,
            faces,
            partial: None,
        }
    }

    #[test]
    fn nonstack_card_types_follow_multiface_layout_rules() {
        let split = definition(Layout::Split, vec![face(&["Instant"]), face(&["Creature"])]);
        assert!(split.matches_card_type_outside_stack(CardTypeFilter::Instant));
        assert!(split.matches_card_type_outside_stack(CardTypeFilter::Creature));
        assert!(!split.matches_card_type_outside_stack(CardTypeFilter::Noncreature));

        for layout in [
            Layout::Adventure,
            Layout::Flip,
            Layout::Transform,
            Layout::ModalDfc,
        ] {
            let card = definition(layout, vec![face(&["Creature"]), face(&["Instant"])]);
            assert!(card.matches_card_type_outside_stack(CardTypeFilter::Creature));
            assert!(!card.matches_card_type_outside_stack(CardTypeFilter::Instant));
            assert!(!card.matches_card_type_outside_stack(CardTypeFilter::InstantOrSorcery));
        }
    }

    #[test]
    fn nonstack_card_names_follow_multiface_layout_rules() {
        let mut left = face(&["Instant"]);
        left.name = "Fire".into();
        let mut right = face(&["Instant"]);
        right.name = "Ice".into();
        let split = definition(Layout::Split, vec![left, right]);
        assert!(split.has_name_outside_stack("Fire"));
        assert!(split.has_name_outside_stack("Ice"));

        let mut front = face(&["Creature"]);
        front.name = "Bonecrusher Giant".into();
        let mut back = face(&["Instant"]);
        back.name = "Stomp".into();
        let adventure = definition(Layout::Adventure, vec![front, back]);
        assert!(adventure.has_name_outside_stack("Bonecrusher Giant"));
        assert!(!adventure.has_name_outside_stack("Stomp"));
    }
}
