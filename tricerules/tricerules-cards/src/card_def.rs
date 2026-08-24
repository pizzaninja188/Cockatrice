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
    ActivatedAbilityDef, AdditionalCost, CardTypeFilter, CastCostGroupDef,
    CastCostReceiptCondition, Color, EffectContext, Evasion, Keyword, PermanentTypeFilter,
    ProtectionQuality, SpellCostModifier, SpellEffectKind, StaticAbilityDef, TargetingDef,
    TriggeredAbilityDef,
};
use serde::{Deserialize, Serialize};

/// One printed mode of a modal spell. Its effects resolve in authored order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeDef {
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
pub struct ModalDef {
    pub min_modes: u32,
    pub max_modes: u32,
    /// Modes in printed order. A mode may be selected at most once in the initial implementation.
    #[serde(default)]
    pub modes: Vec<ModeDef>,
}

impl ModalDef {
    pub(crate) fn validate(&self, context: EffectContext) -> Result<(), String> {
        if self.min_modes == 0
            || self.max_modes < self.min_modes
            || self.max_modes as usize > self.modes.len()
        {
            return Err(format!(
                "modal definition requires 1 <= min_modes <= max_modes <= mode count (got {}..={} with {} modes)",
                self.min_modes,
                self.max_modes,
                self.modes.len()
            ));
        }
        for mode in &self.modes {
            if mode.label.trim().is_empty() {
                return Err("modal mode label must not be empty".into());
            }
            if mode.effects.is_empty() {
                return Err(format!(
                    "modal mode '{}' must contain at least one effect",
                    mode.label
                ));
            }
            for effect in &mode.effects {
                effect.validate(context)?;
            }
            SpellEffectKind::validate_list(&mode.effects)?;
            TargetingDef::validate_optional(mode.targeting.as_ref(), &mode.effects)?;
        }
        Ok(())
    }
}

/// Physical card layout (CR 709/710/712/715/720). Drives how many faces a card has and how each
/// face becomes castable. `Normal` is the overwhelming majority — one face, authored flat.
/// The multi-face variants author [`RawCardDefinition::faces`] explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Layout {
    /// One face, authored with the flat [`RawCardDefinition`] fields.
    #[default]
    Normal,
    /// CR 709: two halves printed side by side, each independently castable (Fire // Ice).
    Split,
    /// CR 709.5: two doors printed side by side. Either door may be cast; on the battlefield
    /// only unlocked doors contribute their characteristics and abilities.
    Room,
    /// CR 712 modal DFC: either face castable from hand; the back is a real card.
    ModalDfc,
    /// CR 712 transforming DFC: front cast, transforms in place to the back (werewolves, Delver).
    Transform,
    /// CR 715: cast the adventure (spell) half, then later the creature half from exile.
    Adventure,
    /// CR 720: cast either the normal permanent characteristics or the inset instant/sorcery
    /// characteristics; a resolving Omen is shuffled into its owner's library.
    Omen,
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
    /// CR 702.180: alternative cost to cast this face from its owner's graveyard.
    #[serde(default)]
    pub harmonize_cost: Option<ManaCost>,
    /// Mandatory nonmana costs paid in addition to this face's mana cost (CR 118.8).
    #[serde(default)]
    pub additional_costs: Vec<AdditionalCost>,
    /// Announced optional/alternative cast-time payments such as kicker and behold.
    #[serde(default)]
    pub cast_cost_groups: Vec<CastCostGroupDef>,
    /// This normally sorcery-speed face may be announced at instant timing only when the named
    /// cast-cost option is selected. Molten Exhale is the calibration consumer.
    #[serde(default)]
    pub instant_speed_cast_cost: Option<CastCostReceiptCondition>,
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
    pub modal_spell: Option<ModalDef>,
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
    /// Parameterized protection abilities printed on this face.
    #[serde(default)]
    pub protections: Vec<ProtectionQuality>,
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
            CardTypeFilter::BasicLand => {
                self.is_land && self.supertypes.iter().any(|value| value == "Basic")
            }
            CardTypeFilter::Land => self.is_land,
            CardTypeFilter::Enchantment => self.is_enchantment,
            CardTypeFilter::Instant => self.is_instant,
            CardTypeFilter::Sorcery => self.is_sorcery,
            CardTypeFilter::InstantOrSorcery => self.is_instant || self.is_sorcery,
            CardTypeFilter::Creature => self.is_creature,
            CardTypeFilter::Artifact => self.is_artifact,
            CardTypeFilter::Planeswalker => self.types.iter().any(|value| value == "Planeswalker"),
            CardTypeFilter::Nonland => !self.is_land,
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
    /// CR 702.180: alternative cost to cast this face from its owner's graveyard.
    #[serde(default)]
    pub harmonize_cost: Option<ManaCost>,
    #[serde(default)]
    pub additional_costs: Vec<AdditionalCost>,
    #[serde(default)]
    pub cast_cost_groups: Vec<CastCostGroupDef>,
    #[serde(default)]
    pub instant_speed_cast_cost: Option<CastCostReceiptCondition>,
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
    pub modal_spell: Option<ModalDef>,
    #[serde(default)]
    pub custom_effect: Option<String>,
    #[serde(default)]
    pub keywords: Vec<Keyword>,
    #[serde(default)]
    pub protections: Vec<ProtectionQuality>,
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
        if self.layout == Layout::Room {
            if self.faces.len() != 2 {
                return Err("Room layout requires exactly two doors".into());
            }
            if self.faces[0].types != self.faces[1].types
                || self.faces[0].supertypes != self.faces[1].supertypes
            {
                return Err("Room doors must have a shared type line".into());
            }
        }
        let faces = if self.faces.is_empty() {
            vec![CardFace {
                name: self.name.clone(),
                mana_cost: self.mana_cost,
                flashback_cost: self.flashback_cost,
                harmonize_cost: self.harmonize_cost,
                additional_costs: self.additional_costs,
                cast_cost_groups: self.cast_cost_groups,
                instant_speed_cast_cost: self.instant_speed_cast_cost,
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
                protections: self.protections,
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
    /// faces as separate entries, while split, Adventure, and Omen cards retain the whole-card entry.
    pub fn face_display_name(&self, face_index: usize) -> Option<&str> {
        let face = self.face(face_index)?;
        Some(match self.layout {
            Layout::Transform | Layout::Flip | Layout::ModalDfc => face.name.as_str(),
            Layout::Normal | Layout::Split | Layout::Room | Layout::Adventure | Layout::Omen => {
                self.name.as_str()
            }
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

    /// CR 709.5: synthesize the battlefield characteristics of a Room from its unlocked
    /// designations. The shared type line remains even with no unlocked doors; every other
    /// characteristic and ability is contributed only by an unlocked door. This is deliberately
    /// an owned, on-demand view so runtime state never caches derived Oracle characteristics.
    pub fn room_permanent_face(&self, unlocked: &[usize]) -> Option<CardFace> {
        if self.layout != Layout::Room {
            return None;
        }
        Self::synthesize_room_permanent_face(&self.faces, unlocked)
    }

    /// The copy-layer form of [`Self::room_permanent_face`]. A copied Room retains its two
    /// copiable door definitions even though its physical registry card has another layout.
    pub fn synthesize_room_permanent_face(
        faces: &[CardFace],
        unlocked: &[usize],
    ) -> Option<CardFace> {
        if faces.len() != 2 {
            return None;
        }

        let mut result = CardFace {
            types: faces[0].types.clone(),
            supertypes: faces[0].supertypes.clone(),
            ..CardFace::default()
        };
        let mut names = Vec::new();
        for index in unlocked.iter().copied() {
            let door = faces.get(index)?;
            names.push(door.name.clone());
            result.mana_cost.pips.extend(door.mana_cost.pips.clone());
            result
                .additional_costs
                .extend(door.additional_costs.clone());
            result
                .cast_cost_groups
                .extend(door.cast_cost_groups.clone());
            result.instant_speed_cast_cost = result
                .instant_speed_cast_cost
                .or(door.instant_speed_cast_cost);
            result.cost_modifiers.extend(door.cost_modifiers.clone());
            result.spell_effect.extend(door.spell_effect.clone());
            result.keywords.extend(door.keywords.clone());
            result.protections.extend(door.protections.clone());
            result.evasions.extend(door.evasions.clone());
            result
                .activated_abilities
                .extend(door.activated_abilities.clone());
            result
                .triggered_abilities
                .extend(door.triggered_abilities.clone());
            result
                .static_abilities
                .extend(door.static_abilities.clone());
            result.must_attack_if_able |= door.must_attack_if_able;
            result.must_block_if_able |= door.must_block_if_able;
            if let Some(indicator) = &door.color_indicator {
                let combined = result.color_indicator.get_or_insert_with(Vec::new);
                for color in indicator {
                    if !combined.contains(color) {
                        combined.push(*color);
                    }
                }
            }
        }
        result.name = names.join(" // ");
        result.derive_type_flags();
        Some(result)
    }

    /// True if this card has more than one face (any non-`Normal` layout).
    pub fn is_multiface(&self) -> bool {
        self.faces.len() > 1
    }

    /// Whether this physical card matches `filter` in a zone other than the battlefield or stack.
    /// Split cards combine both halves (CR 709.4). Flip, double-faced, and adventurer cards use
    /// only their normal/front characteristics there (CR 710.2, 712.8a, 715.4).
    pub fn matches_card_type_outside_stack(&self, filter: CardTypeFilter) -> bool {
        if !matches!(self.layout, Layout::Split | Layout::Room) {
            return self.primary_face().matches_card_type(filter);
        }

        match filter {
            CardTypeFilter::Nonland => self.faces_iter().all(|face| !face.is_land),
            CardTypeFilter::Noncreature => self.faces_iter().all(|face| !face.is_creature),
            _ => self.faces_iter().any(|face| face.matches_card_type(filter)),
        }
    }

    /// The card types this physical card has outside the battlefield and stack. Split cards
    /// combine both halves; other multiface layouts use only their normal/front face.
    pub fn card_types_outside_stack(&self) -> Vec<&str> {
        const CARD_TYPES: [&str; 9] = [
            "Artifact",
            "Battle",
            "Creature",
            "Enchantment",
            "Instant",
            "Kindred",
            "Land",
            "Planeswalker",
            "Sorcery",
        ];

        CARD_TYPES
            .into_iter()
            .filter(|card_type| {
                if matches!(self.layout, Layout::Split | Layout::Room) {
                    self.faces_iter()
                        .any(|face| face.types.iter().any(|value| value == card_type))
                } else {
                    self.primary_face()
                        .types
                        .iter()
                        .any(|value| value == card_type)
                }
            })
            .collect()
    }

    /// The physical card's mana value outside the stack. Split cards and Rooms use the sum of
    /// both halves/doors (CR 202.3d); other multiface layouts use only their front face.
    pub fn mana_value_outside_stack(&self) -> u32 {
        if matches!(self.layout, Layout::Split | Layout::Room) {
            self.faces_iter()
                .map(|face| face.mana_cost.mana_value())
                .sum()
        } else {
            self.primary_face().mana_cost.mana_value()
        }
    }

    /// Whether this physical card has `name` in a zone other than the battlefield or stack.
    /// Split cards have both half names there (CR 709.4); flip, double-faced, and adventurer cards
    /// use only their normal/front face (CR 710.2, 712.8a, 715.4).
    pub fn has_name_outside_stack(&self, name: &str) -> bool {
        if matches!(self.layout, Layout::Split | Layout::Room) {
            self.faces_iter().any(|face| face.name == name)
        } else {
            self.primary_face().name == name
        }
    }

    /// Whether this physical card has `subtype` outside the battlefield and stack. Split cards
    /// combine both halves; other multiface layouts use only their normal/front face.
    pub fn has_subtype_outside_stack(&self, subtype: &str) -> bool {
        if matches!(self.layout, Layout::Split | Layout::Room) {
            self.faces_iter()
                .any(|face| face.types.iter().any(|value| value == subtype))
        } else {
            self.primary_face()
                .types
                .iter()
                .any(|value| value == subtype)
        }
    }

    /// Whether `face_index` is a face the player may choose while playing this card from hand.
    /// Split cards, modal DFCs, Adventures, and Omens expose both authored spell/land choices there;
    /// transforming DFCs and flip cards expose only their front/top face. This is a layout rule,
    /// independent of timing, costs, targets, and whether the chosen face is a spell or land.
    pub fn face_available_from_hand(&self, face_index: usize) -> bool {
        match self.layout {
            Layout::Normal | Layout::Transform | Layout::Flip => face_index == 0,
            Layout::Split | Layout::Room | Layout::ModalDfc | Layout::Adventure | Layout::Omen => {
                face_index < self.face_count()
            }
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
        assert_eq!(
            split.card_types_outside_stack(),
            vec!["Creature", "Instant"]
        );

        for layout in [
            Layout::Adventure,
            Layout::Omen,
            Layout::Flip,
            Layout::Transform,
            Layout::ModalDfc,
        ] {
            let card = definition(layout, vec![face(&["Creature"]), face(&["Instant"])]);
            assert!(card.matches_card_type_outside_stack(CardTypeFilter::Creature));
            assert!(!card.matches_card_type_outside_stack(CardTypeFilter::Instant));
            assert!(!card.matches_card_type_outside_stack(CardTypeFilter::InstantOrSorcery));
            assert_eq!(card.card_types_outside_stack(), vec!["Creature"]);
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

        let mut normal = face(&["Creature", "Dragon"]);
        normal.name = "Sagu Wildling".into();
        let mut omen = face(&["Sorcery", "Omen"]);
        omen.name = "Roost Seek".into();
        let omen_card = definition(Layout::Omen, vec![normal, omen]);
        assert!(omen_card.has_name_outside_stack("Sagu Wildling"));
        assert!(!omen_card.has_name_outside_stack("Roost Seek"));
    }

    #[test]
    fn room_has_combined_characteristics_outside_battlefield_and_stack() {
        let mut left = face(&["Enchantment", "Room"]);
        left.name = "Ticket Booth".into();
        left.mana_cost = ManaCost::parse("{2}{R}").unwrap();
        let mut right = face(&["Enchantment", "Room"]);
        right.name = "Tunnel of Hate".into();
        right.mana_cost = ManaCost::parse("{4}{R}{R}").unwrap();
        let room = definition(Layout::Room, vec![left, right]);

        assert!(room.matches_card_type_outside_stack(CardTypeFilter::Enchantment));
        assert_eq!(room.card_types_outside_stack(), vec!["Enchantment"]);
        assert!(room.has_name_outside_stack("Ticket Booth"));
        assert!(room.has_name_outside_stack("Tunnel of Hate"));
        assert!(room.has_subtype_outside_stack("Room"));
        assert_eq!(room.mana_value_outside_stack(), 9);
    }

    #[test]
    fn room_permanent_face_contains_only_unlocked_door_abilities() {
        let mut left = face(&["Enchantment", "Room"]);
        left.name = "Left".into();
        left.keywords = vec![Keyword::Flying];
        let mut right = face(&["Enchantment", "Room"]);
        right.name = "Right".into();
        right.keywords = vec![Keyword::Vigilance];
        let room = definition(Layout::Room, vec![left, right]);

        let locked = room.room_permanent_face(&[]).unwrap();
        assert_eq!(locked.types, vec!["Enchantment", "Room"]);
        assert!(locked.name.is_empty());
        assert!(locked.keywords.is_empty());

        let both = room.room_permanent_face(&[0, 1]).unwrap();
        assert_eq!(both.name, "Left // Right");
        assert_eq!(both.keywords, vec![Keyword::Flying, Keyword::Vigilance]);
    }
}
