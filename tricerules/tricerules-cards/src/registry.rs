use crate::card_def::{CardDefinition, Layout, RawCardDefinition};
use crate::primitives::{EffectContext, SpellEffectKind, StaticAbilityDef};
use crate::token_def::TokenDefinition;
use once_cell::sync::Lazy;
use ron::extensions::Extensions;
use ron::Options;
use std::collections::HashMap;
use thiserror::Error;

/// `Option` fields need `IMPLICIT_SOME` so bare values (e.g. `2` for `Option<u32>`) deserialize.
static RON_OPTS: Lazy<Options> =
    Lazy::new(|| Options::default().with_default_extension(Extensions::IMPLICIT_SOME));

/// Parsed once per process and shared by every game (read-only after init).
/// Panics on invalid embedded data: fail-fast at sidecar startup is the validation point.
static GLOBAL: Lazy<CardRegistry> =
    Lazy::new(|| CardRegistry::from_embedded().expect("embedded card data"));

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("ron parse: {0}")]
    Ron(#[from] ron::error::SpannedError),
    #[error("invalid card data for '{id}': {reason}")]
    InvalidCard { id: String, reason: String },
}

#[derive(Debug, Default)]
pub struct CardRegistry {
    by_id: HashMap<String, CardDefinition>,
    /// Trimmed, lowercased Oracle name -> card id (see [`Self::id_for_name`]).
    by_name: HashMap<String, String>,
    /// Token namespace: token id -> the [`CardDefinition`] synthesized from its
    /// [`TokenDefinition`] (CR 111). Kept apart from `by_id` so tokens are never deck cards
    /// or counted as implemented Oracle cards, but [`Self::get`] falls back here so the engine's
    /// characteristic queries work uniformly for token objects.
    tokens: HashMap<String, CardDefinition>,
}

/// Name-index key normalization, applied to both stored names and lookup queries.
fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}

impl CardRegistry {
    pub fn from_embedded() -> Result<Self, RegistryError> {
        Self::from_chunks_and_tokens(EMBEDDED_RON_CHUNKS, EMBEDDED_TOKEN_CHUNKS)
    }

    #[cfg(test)]
    fn from_chunks(chunks: &[&str]) -> Result<Self, RegistryError> {
        Self::from_chunks_and_tokens(chunks, &[])
    }

    fn from_chunks_and_tokens(
        chunks: &[&str],
        token_chunks: &[&str],
    ) -> Result<Self, RegistryError> {
        let mut reg = CardRegistry::default();
        // Tokens first: card effects (CreateTokens) are validated against the token namespace.
        for chunk in token_chunks {
            let token: TokenDefinition = RON_OPTS.from_str(chunk)?;
            let mut def = token.to_card_def();
            def.derive_type_flags();
            let id = token.id.clone();
            if reg.tokens.insert(id.clone(), def).is_some() {
                return Err(RegistryError::InvalidCard {
                    id,
                    reason: "duplicate token id".into(),
                });
            }
        }
        for chunk in chunks {
            // Authored RON (flat for single-face cards) is normalized into the faces-only runtime
            // shape here — the one place that knows about the flat authoring schema.
            let raw: RawCardDefinition = RON_OPTS.from_str(chunk)?;
            let id = raw.id.clone();
            let mut card = raw
                .into_definition()
                .map_err(|reason| RegistryError::InvalidCard { id, reason })?;
            // Type flags are derived from `types`/`supertypes`, not authored in RON (per face).
            card.derive_type_flags();
            if card.layout == Layout::Adventure {
                let valid_roles = card.faces.len() == 2
                    && card.faces[0].is_permanent()
                    && (card.faces[1].is_instant || card.faces[1].is_sorcery)
                    && !card.faces[1].is_permanent();
                if !valid_roles {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "Adventure requires exactly two faces: permanent face 0 and instant/sorcery face 1"
                            .into(),
                    });
                }
            }
            // Validate every face's effects at startup — multi-face cards (CR 709/712/715)
            // validate each face uniformly. Spell effects have no source permanent, so `Source`
            // subjects are rejected here (EffectContext::Spell); activated/triggered
            // effects bind to a source (Ability).
            for face in card.faces_iter() {
                // One resolution owner per face (CR 608): ordinary data, modal data, and a
                // custom (tier-3) effect are mutually exclusive. The
                // matching custom impl is validated to exist on the `tricerules-core` side
                // (it owns the `CardEffect` lookup; this crate has no engine access).
                let resolution_owners = usize::from(!face.spell_effect.is_empty())
                    + usize::from(face.modal_spell.is_some())
                    + usize::from(face.custom_effect.is_some());
                if resolution_owners > 1 {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "face has more than one of spell_effect, modal_spell, and \
                                 custom_effect (one resolution owner allowed)"
                            .into(),
                    });
                }
                for effect in &face.spell_effect {
                    effect.validate(EffectContext::Spell).map_err(|reason| {
                        RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        }
                    })?;
                }
                // Rules that depend on sibling effects (e.g. an amount read from another
                // effect's target) can only be checked over the whole list.
                SpellEffectKind::validate_list(&face.spell_effect).map_err(|reason| {
                    RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    }
                })?;
                if let Some(modal) = &face.modal_spell {
                    if modal.min_modes == 0
                        || modal.max_modes < modal.min_modes
                        || modal.max_modes as usize > modal.modes.len()
                    {
                        return Err(RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason: format!(
                                "modal_spell requires 1 <= min_modes <= max_modes <= mode count \
                                 (got {}..={} with {} modes)",
                                modal.min_modes,
                                modal.max_modes,
                                modal.modes.len()
                            ),
                        });
                    }
                    for mode in &modal.modes {
                        if mode.label.trim().is_empty() {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: "modal_spell mode label must not be empty".into(),
                            });
                        }
                        if mode.effects.is_empty() {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: format!(
                                    "modal_spell mode '{}' must contain at least one effect",
                                    mode.label
                                ),
                            });
                        }
                        for effect in &mode.effects {
                            effect.validate(EffectContext::Spell).map_err(|reason| {
                                RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason,
                                }
                            })?;
                        }
                        // A mode's effects are the resolution list for that mode (CR 700.2),
                        // so the sibling-dependent rules apply per mode.
                        SpellEffectKind::validate_list(&mode.effects).map_err(|reason| {
                            RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            }
                        })?;
                    }
                }
                // CR 604.2: static abilities exist only on permanents (they generate continuous
                // effects while the source is on the battlefield). An instant/sorcery with one is
                // invalid data — its "anthem" belongs in `spell_effect` as a one-shot `PumpAll`.
                if !face.static_abilities.is_empty() && (face.is_instant || face.is_sorcery) {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason:
                            "static_abilities are only valid on permanents (not instant/sorcery)"
                                .into(),
                    });
                }
                let spell_aura_attach_count = face
                    .spell_effect
                    .iter()
                    .filter(|effect| matches!(effect, SpellEffectKind::AuraAttach { .. }))
                    .count();
                let nonspell_aura_attach = face
                    .activated_abilities
                    .iter()
                    .flat_map(|ability| &ability.effect)
                    .chain(
                        face.triggered_abilities
                            .iter()
                            .flat_map(|ability| &ability.effect),
                    )
                    .chain(
                        face.modal_spell
                            .iter()
                            .flat_map(|modal| &modal.modes)
                            .flat_map(|mode| &mode.effects),
                    )
                    .any(|effect| matches!(effect, SpellEffectKind::AuraAttach { .. }));
                if face.is_aura && (spell_aura_attach_count != 1 || nonspell_aura_attach) {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "an Aura face requires exactly one AuraAttach in spell_effect"
                            .into(),
                    });
                }
                if !face.is_aura && (spell_aura_attach_count != 0 || nonspell_aura_attach) {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "AuraAttach is only valid on an Aura face".into(),
                    });
                }
                let attachment_source = face.is_aura || face.types.iter().any(|t| t == "Equipment");
                for ability in &face.static_abilities {
                    if let StaticAbilityDef::AttachedModifier {
                        delta_power,
                        delta_toughness,
                        keywords,
                        cant_attack,
                        cant_block,
                    } = ability
                    {
                        if !attachment_source {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: "AttachedModifier requires an Aura or Equipment source"
                                    .into(),
                            });
                        }
                        if *delta_power == 0
                            && *delta_toughness == 0
                            && keywords.is_empty()
                            && !cant_attack
                            && !cant_block
                        {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: "AttachedModifier must modify at least one value".into(),
                            });
                        }
                    }
                }
                // An ability's effect list gets the same two checks a spell's does: each effect
                // against its context, then the list as a whole (CR 608.2 — the effects resolve
                // together, so a cross-effect requirement like `LoseLife(TargetManaValue)` must
                // find its object-targeting sibling inside this one ability).
                for effects in face
                    .activated_abilities
                    .iter()
                    .map(|a| &a.effect)
                    .chain(face.triggered_abilities.iter().map(|t| &t.effect))
                {
                    for effect in effects {
                        effect.validate(EffectContext::Ability).map_err(|reason| {
                            RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            }
                        })?;
                    }
                    SpellEffectKind::validate_list(effects).map_err(|reason| {
                        RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        }
                    })?;
                }
                // Every CreateTokens effect must name a loaded token (an uncreatable id is a bug).
                let all_effects = face
                    .spell_effect
                    .iter()
                    .chain(face.activated_abilities.iter().flat_map(|a| &a.effect))
                    .chain(face.triggered_abilities.iter().flat_map(|t| &t.effect));
                for effect in all_effects {
                    if let SpellEffectKind::CreateTokens { token, .. } = effect {
                        if !reg.tokens.contains_key(token) {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: format!("CreateTokens references unknown token '{token}'"),
                            });
                        }
                    }
                }
                if let Some(modal) = &face.modal_spell {
                    for effect in modal.modes.iter().flat_map(|mode| &mode.effects) {
                        if let SpellEffectKind::CreateTokens { token, .. } = effect {
                            if !reg.tokens.contains_key(token) {
                                return Err(RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason: format!(
                                        "CreateTokens references unknown token '{token}'"
                                    ),
                                });
                            }
                        }
                    }
                }
            }
            let id = card.id.clone();
            // Index the whole-card name (what decks/`cards.xml` reference) and, for multi-face
            // cards, each face name, so `id_for_name` resolves either half to the one card id.
            let mut names: Vec<String> = vec![card.name.clone()];
            if card.is_multiface() {
                names.extend(card.faces.iter().map(|f| f.name.clone()));
            }
            for name in names {
                if reg
                    .by_name
                    .insert(normalize_name(&name), id.clone())
                    .is_some()
                {
                    return Err(RegistryError::InvalidCard {
                        id,
                        reason: format!("duplicate name '{name}'"),
                    });
                }
            }
            if reg.by_id.insert(id.clone(), card).is_some() {
                return Err(RegistryError::InvalidCard {
                    id,
                    reason: "duplicate id".into(),
                });
            }
        }
        Ok(reg)
    }

    /// Look up a definition by id. Falls back to the token namespace so the engine queries a
    /// token object's characteristics (types, P/T, keywords, colors) the same way as a card.
    pub fn get(&self, id: &str) -> Option<&CardDefinition> {
        self.by_id.get(id).or_else(|| self.tokens.get(id))
    }

    /// True if `id` names a token (created by an effect), not a deck card.
    pub fn is_token(&self, id: &str) -> bool {
        self.tokens.contains_key(id)
    }

    /// Resolves an Oracle card name (trimmed, case-insensitive) to a card id.
    /// This is the only supported name->id path; deck lists cross IPC as names.
    pub fn id_for_name(&self, name: &str) -> Option<&str> {
        self.by_name.get(&normalize_name(name)).map(String::as_str)
    }

    /// Iterate over every loaded card definition (order is unspecified).
    pub fn definitions(&self) -> impl Iterator<Item = &CardDefinition> {
        self.by_id.values()
    }

    pub fn global() -> &'static CardRegistry {
        &GLOBAL
    }

    /// Stable FNV-1a hash of the sorted embedded card RON, as 16-char hex.
    /// Build-to-build stable (not cryptographic): a card-data version tag for the
    /// Servatrice↔sidecar handshake and ruled-replay stamping, so (seed, command log,
    /// data hash) reproduces a game. `EMBEDDED_RON_CHUNKS` is path-sorted by build.rs,
    /// so the hash is independent of filesystem enumeration order.
    pub fn content_hash() -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a offset basis
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        // Tokens are part of the rules data, so fold them into the hash after cards.
        for chunk in EMBEDDED_RON_CHUNKS
            .iter()
            .chain(EMBEDDED_TOKEN_CHUNKS.iter())
        {
            for &b in chunk.as_bytes() {
                hash ^= b as u64;
                hash = hash.wrapping_mul(PRIME);
            }
            hash ^= 0xff; // chunk separator so file boundaries are significant
            hash = hash.wrapping_mul(PRIME);
        }
        format!("{hash:016x}")
    }
}

// `EMBEDDED_RON_CHUNKS`, generated by build.rs from `data/**/*.ron`.
include!(concat!(env!("OUT_DIR"), "/embedded_cards.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::{Amount, SpellEffectKind, TargetFilter, TargetKind};

    #[test]
    fn embedded_registry_loads() {
        CardRegistry::from_embedded().unwrap();
    }

    #[test]
    fn spell_effects_deserialize_from_ron() {
        let reg = CardRegistry::from_embedded().unwrap();
        assert_eq!(
            reg.get("angels_mercy").unwrap().primary_face().spell_effect,
            vec![SpellEffectKind::GainLife {
                amount: Amount::Fixed(7)
            }]
        );
        assert_eq!(
            reg.get("lightning_bolt")
                .unwrap()
                .primary_face()
                .spell_effect,
            vec![SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(3),
                target: TargetFilter {
                    kind: TargetKind::AnyTarget,
                    ..Default::default()
                },
            }]
        );
        assert_eq!(
            reg.get("mind_sculpt").unwrap().primary_face().spell_effect,
            vec![SpellEffectKind::MillTargetPlayer {
                count: 7,
                target: TargetFilter {
                    kind: TargetKind::OpponentPlayer,
                    ..Default::default()
                },
            }]
        );
    }

    #[test]
    fn startup_validation_rejects_incompatible_target_filter() {
        // A player-life effect pointed at a creature is invalid card data.
        let bad = r#"(
            id: "bad_card",
            name: "Bad Card",
            mana_cost: "{W}",
            types: ["Instant"],
            spell_effect: [TargetPlayerGainsLife(amount: 3, target: (kind: Creature))],
        )"#;
        let raw: RawCardDefinition = RON_OPTS.from_str(bad).unwrap();
        let card = raw.into_definition().unwrap();
        assert!(card.primary_face().spell_effect[0]
            .validate(crate::primitives::EffectContext::Spell)
            .is_err());
    }

    /// CR 701.19 / 701.20: tapping and untapping act on permanents, so a player-kind filter on
    /// either is invalid card data and must not survive registry load.
    #[test]
    fn load_rejects_tap_or_untap_aimed_at_a_player() {
        for effect in ["TapTarget", "UntapTarget"] {
            let bad = format!(
                r#"(
            id: "bad_{}",
            name: "Bad {}",
            mana_cost: "{{U}}",
            types: ["Instant"],
            spell_effect: [{}(target: (kind: AnyPlayer))],
        )"#,
                effect.to_lowercase(),
                effect,
                effect
            );
            let err = CardRegistry::from_chunks(&[&bad]).unwrap_err();
            assert!(
                matches!(err, RegistryError::InvalidCard { .. }),
                "{effect} at a player should be rejected, got {err:?}"
            );
        }
    }

    /// `LoseLife(amount: TargetManaValue)` reads a sibling effect's target, so a list without
    /// an object-targeting effect is invalid data — it would silently resolve to 0 life.
    #[test]
    fn load_rejects_target_mana_value_without_an_object_target() {
        let bad = r#"(
            id: "bad_lose_life",
            name: "Bad Lose Life",
            mana_cost: "{B}",
            types: ["Sorcery"],
            spell_effect: [LoseLife(amount: TargetManaValue)],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(matches!(err, RegistryError::InvalidCard { ref id, .. } if id == "bad_lose_life"));

        // A player target is not enough either — players have no mana value.
        let bad_player_target = r#"(
            id: "bad_lose_life_player",
            name: "Bad Lose Life Player",
            mana_cost: "{B}",
            types: ["Sorcery"],
            spell_effect: [
                MillTargetPlayer(count: 2, target: (kind: AnyPlayer)),
                LoseLife(amount: TargetManaValue),
            ],
        )"#;
        let err = CardRegistry::from_chunks(&[bad_player_target]).unwrap_err();
        assert!(
            matches!(err, RegistryError::InvalidCard { ref id, .. } if id == "bad_lose_life_player")
        );

        // Paired with a graveyard-card target (Reanimate's shape) it loads.
        let good = r#"(
            id: "good_lose_life",
            name: "Good Lose Life",
            mana_cost: "{B}",
            types: ["Sorcery"],
            spell_effect: [
                ReturnFromGraveyard(filter: (owner: AnyPlayer, card_type: Some(Creature)), destination: Battlefield),
                LoseLife(amount: TargetManaValue),
            ],
        )"#;
        assert!(CardRegistry::from_chunks(&[good]).is_ok());

        // A fixed amount never needs a target.
        let fixed = r#"(
            id: "fixed_lose_life",
            name: "Fixed Lose Life",
            mana_cost: "{B}",
            types: ["Sorcery"],
            spell_effect: [LoseLife(amount: Fixed(2))],
        )"#;
        assert!(CardRegistry::from_chunks(&[fixed]).is_ok());
    }

    #[test]
    fn load_rejects_invalid_triggered_ability_effect() {
        let bad = r#"(
            id: "bad_trigger",
            name: "Bad Trigger",
            mana_cost: "{G}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [
                (
                    trigger: WhenSelfEntersBattlefield,
                    effect: [TargetPlayerGainsLife(amount: 3, target: (kind: Creature))],
                    text: "bad",
                ),
            ],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(matches!(err, RegistryError::InvalidCard { ref id, .. } if id == "bad_trigger"));
    }

    /// CR 608.2: an ability may carry several effects, resolved in written order — the same
    /// shape as a spell's `spell_effect`. Both ability kinds accept a list, and the per-effect
    /// context validation still runs on every element (the `Source`-in-a-spell rejection below
    /// proves the per-element half).
    #[test]
    fn abilities_accept_multiple_effects() {
        let card = r#"(
            id: "multi_effect",
            name: "Multi Effect",
            mana_cost: "{B}",
            types: ["Enchantment"],
            activated_abilities: [
                (
                    cost: Mana("{1}"),
                    effect: [Draw(count: 1), LoseLife(amount: Fixed(1))],
                    text: "{1}: Draw a card and lose 1 life.",
                ),
            ],
            triggered_abilities: [
                (
                    trigger: WhenSelfEntersBattlefield,
                    effect: [GainLife(amount: 2), Draw(count: 1)],
                    text: "When this enters, gain 2 life and draw a card.",
                ),
            ],
        )"#;
        let reg = CardRegistry::from_chunks(&[card]).expect("multi-effect abilities load");
        let def = reg.get("multi_effect").expect("card present");
        let face = def.primary_face();
        assert_eq!(face.activated_abilities[0].effect.len(), 2);
        assert_eq!(face.triggered_abilities[0].effect.len(), 2);
        // Order is the authored order — the engine resolves the list front to back.
        assert!(matches!(
            face.triggered_abilities[0].effect[0],
            SpellEffectKind::GainLife { .. }
        ));
    }

    /// A many-effect ability is *not* a mana ability (CR 605.1a) even if one of its effects
    /// produces mana: the fast no-stack path is reserved for the sole-effect case.
    #[test]
    fn mana_options_requires_produce_mana_to_be_the_only_effect() {
        let card = r#"(
            id: "impure_mana",
            name: "Impure Mana",
            mana_cost: "{1}",
            types: ["Artifact"],
            activated_abilities: [
                (
                    cost: Tap,
                    effect: [ProduceMana(options: [(c: 1)]), LoseLife(amount: Fixed(1))],
                    text: "{T}: Add {C}. You lose 1 life.",
                ),
                (
                    cost: Tap,
                    effect: [ProduceMana(options: [(c: 1)])],
                    text: "{T}: Add {C}.",
                ),
            ],
        )"#;
        let reg = CardRegistry::from_chunks(&[card]).expect("card loads");
        let face = reg.get("impure_mana").unwrap().primary_face();
        assert!(face.activated_abilities[0].mana_options().is_none());
        assert!(face.activated_abilities[1].mana_options().is_some());
    }

    /// A self-pump trigger (the replacement for the old `TriggeredEffect::PumpSelf`) loads,
    /// while the same `Source` subject in a spell's effect list is rejected at load.
    #[test]
    fn self_pump_trigger_loads_but_self_in_spell_rejected() {
        let good = r#"(
            id: "self_pumper",
            name: "Self Pumper",
            mana_cost: "{G}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [
                (
                    trigger: AtBeginningOfUpkeep(player: Controller),
                    effect: [PumpTarget(power: 1, toughness: 1, subject: Source)],
                    text: "At the beginning of your upkeep, this gets +1/+1.",
                ),
            ],
        )"#;
        assert!(CardRegistry::from_chunks(&[good]).is_ok());

        let bad = r#"(
            id: "self_spell",
            name: "Self Spell",
            mana_cost: "{G}",
            types: ["Instant"],
            spell_effect: [PumpTarget(power: 1, toughness: 1, subject: Source)],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(matches!(err, RegistryError::InvalidCard { ref id, .. } if id == "self_spell"));
    }

    /// Authoring convention (not a wire contract): every card's id is the slug of its name.
    /// Catches id/name typos in current and future RON; Phase 6 codegen reuses `slugify`.
    #[test]
    fn card_ids_follow_slug_convention() {
        let reg = CardRegistry::from_embedded().unwrap();
        for def in reg.definitions() {
            assert_eq!(
                def.id,
                crate::slug::slugify(&def.name),
                "card '{}': id does not match slugify(name)",
                def.name
            );
        }
    }

    /// CR 709/712/715: a multi-face card loads with a `faces` vec, resolves to the card id by
    /// either its whole-card `//` name or either face name, and each face exposes its own
    /// characteristics. The slug invariant (tested above) covers the `//` whole-card name.
    #[test]
    fn multiface_card_loads_and_resolves_by_face_name() {
        let reg = CardRegistry::from_embedded().unwrap();
        let def = reg.get("fire_ice").expect("fire_ice loaded");
        assert_eq!(def.face_count(), 2);
        assert!(def.is_multiface());
        // Whole-card name and both face names all resolve to the one card id.
        assert_eq!(reg.id_for_name("Fire // Ice"), Some("fire_ice"));
        assert_eq!(reg.id_for_name("Fire"), Some("fire_ice"));
        assert_eq!(reg.id_for_name("Ice"), Some("fire_ice"));
        // Each face carries its own cost/types.
        let fire = def.face(0).unwrap();
        let ice = def.face(1).unwrap();
        assert_eq!(fire.name, "Fire");
        assert_eq!(ice.name, "Ice");
        assert!(fire.is_instant && ice.is_instant);
        assert!(!fire.is_permanent() && !ice.is_permanent());
    }

    /// The flat authoring schema (`mana_cost`/`types`/… at the top level) is normalized into
    /// `faces[0]` at load, so the runtime definition is faces-only for single-face cards too.
    #[test]
    fn flat_authoring_becomes_a_single_face() {
        let reg = CardRegistry::from_embedded().unwrap();
        let def = reg.get("grizzly_bears").expect("grizzly_bears loaded");
        assert_eq!(def.face_count(), 1);
        assert!(!def.is_multiface());
        let face = def.primary_face();
        assert_eq!(face.name, def.name);
        assert_eq!(face.mana_cost.to_string(), "{1}{G}");
        assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
        assert!(face.is_creature && face.is_permanent());
    }

    /// The layout and the authored `faces` list must agree: a multi-face layout has to author
    /// faces, and a `Normal` card must not (its lone face is the flat fields).
    #[test]
    fn load_rejects_layout_and_faces_disagreement() {
        let faceless_split = r#"(
            id: "faceless_split",
            name: "Faceless Split",
            layout: Split,
            mana_cost: "{R}",
            types: ["Instant"],
        )"#;
        let err = CardRegistry::from_chunks(&[faceless_split]).unwrap_err();
        match err {
            RegistryError::InvalidCard { id, reason } => {
                assert_eq!(id, "faceless_split");
                assert!(reason.contains("requires an authored `faces` list"));
            }
            other => panic!("expected InvalidCard, got {other:?}"),
        }

        let normal_with_faces = r#"(
            id: "normal_with_faces",
            name: "Normal With Faces",
            faces: [
                (name: "A", mana_cost: "{R}", types: ["Instant"]),
                (name: "B", mana_cost: "{U}", types: ["Instant"]),
            ],
        )"#;
        let err = CardRegistry::from_chunks(&[normal_with_faces]).unwrap_err();
        match err {
            RegistryError::InvalidCard { id, reason } => {
                assert_eq!(id, "normal_with_faces");
                assert!(reason.contains("Normal-layout card"));
            }
            other => panic!("expected InvalidCard, got {other:?}"),
        }
    }

    #[test]
    fn id_for_name_normalizes_trim_and_case() {
        let reg = CardRegistry::from_embedded().unwrap();
        assert_eq!(reg.id_for_name("Lightning Bolt"), Some("lightning_bolt"));
        assert_eq!(reg.id_for_name("  lightning BOLT "), Some("lightning_bolt"));
        assert_eq!(reg.id_for_name("Pharika's Chosen"), Some("pharikas_chosen"));
        assert_eq!(reg.id_for_name("Black Lotus"), None);
    }

    #[test]
    fn load_rejects_duplicate_name() {
        let a = r#"(
            id: "dupe_a",
            name: "Dupe",
            mana_cost: "",
            types: ["Land"],
        )"#;
        let b = r#"(
            id: "dupe_b",
            name: " DUPE ",
            mana_cost: "",
            types: ["Land"],
        )"#;
        let err = CardRegistry::from_chunks(&[a, b]).unwrap_err();
        match err {
            RegistryError::InvalidCard { id, reason } => {
                assert_eq!(id, "dupe_b");
                assert!(reason.contains("duplicate name"));
            }
            other => panic!("expected InvalidCard, got {other:?}"),
        }
    }

    #[test]
    fn load_rejects_duplicate_id() {
        let card = r#"(
            id: "dupe",
            name: "Dupe",
            mana_cost: "",
            types: ["Land"],
        )"#;
        let err = CardRegistry::from_chunks(&[card, card]).unwrap_err();
        match err {
            RegistryError::InvalidCard { id, reason } => {
                assert_eq!(id, "dupe");
                assert!(reason.contains("duplicate"));
            }
            other => panic!("expected InvalidCard, got {other:?}"),
        }
    }

    #[test]
    fn tokens_load_into_separate_namespace_with_explicit_colors() {
        use crate::primitives::Color;
        let reg = CardRegistry::from_embedded().unwrap();
        // A token resolves through get() but is flagged as a token and is not a deck card.
        let soldier = reg.get("soldier_w_1_1").expect("soldier token");
        assert!(reg.is_token("soldier_w_1_1"));
        assert!(!reg.is_token("lightning_bolt"));
        let face = soldier.primary_face();
        assert!(face.is_creature);
        assert_eq!((face.power, face.toughness), (Some(1), Some(1)));
        // CR 111.4: color comes from the creating effect, not a (nonexistent) mana cost.
        assert_eq!(face.colors(), vec![Color::White]);
        // Tokens never appear in the name index or the implemented-card iterator.
        assert_eq!(reg.id_for_name("Soldier"), None);
        assert!(reg.definitions().all(|d| !reg.is_token(&d.id)));
    }

    #[test]
    fn token_ids_extend_name_slug() {
        // A token's name is just its subtype ("Soldier"), but its identity (CR 111.4) is the full
        // characteristic tuple, so several distinct tokens can share a name. Each therefore gets a
        // descriptive id of the form `<name-slug>[_<characteristics...>]` (e.g. `soldier_w_1_1`,
        // `soldier_w_1_1_lifelink`). We can't require id == slugify(name) (that allows only one
        // token per name); instead require slugify(name) to be the id's leading segment, keeping
        // the id traceable to the name. Uniqueness is enforced at load (duplicate token id error).
        let reg = CardRegistry::from_embedded().unwrap();
        for (id, def) in &reg.tokens {
            let slug = crate::slug::slugify(&def.name);
            let ok = *id == slug
                || id
                    .strip_prefix(&slug)
                    .is_some_and(|rest| rest.starts_with('_'));
            assert!(
                ok,
                "token '{}': id '{id}' must be '{slug}' or start with '{slug}_'",
                def.name
            );
        }
    }

    #[test]
    fn create_tokens_referencing_unknown_token_rejected() {
        let bad = r#"(
            id: "bad_maker",
            name: "Bad Maker",
            mana_cost: "{W}",
            types: ["Sorcery"],
            spell_effect: [CreateTokens(token: "no_such_token", count: 1)],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(matches!(err, RegistryError::InvalidCard { ref id, .. } if id == "bad_maker"));
    }

    #[test]
    fn load_rejects_card_with_both_spell_effect_and_custom_effect() {
        // A face may have exactly one resolution owner (tier-1/2 data or tier-3 custom).
        let bad = r#"(
            id: "double_owner",
            name: "Double Owner",
            mana_cost: "{U}",
            types: ["Instant"],
            spell_effect: [Draw(count: 1)],
            custom_effect: "brainstorm",
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        match err {
            RegistryError::InvalidCard { id, reason } => {
                assert_eq!(id, "double_owner");
                assert!(reason.contains("one resolution owner"));
            }
            other => panic!("expected InvalidCard, got {other:?}"),
        }
    }

    #[test]
    fn modal_spell_deserializes_and_validates() {
        let good = r#"(
            id: "modal_test",
            name: "Modal Test",
            mana_cost: "{W}",
            types: ["Instant"],
            modal_spell: (
                min_modes: 1,
                max_modes: 2,
                modes: [
                    (label: "Gain life", effects: [GainLife(amount: 3)]),
                    (label: "Draw a card", effects: [Draw(count: 1)]),
                ],
            ),
        )"#;
        let registry = CardRegistry::from_chunks(&[good]).unwrap();
        let modal = registry
            .get("modal_test")
            .unwrap()
            .primary_face()
            .modal_spell
            .as_ref()
            .unwrap();
        assert_eq!((modal.min_modes, modal.max_modes), (1, 2));
        assert_eq!(modal.modes.len(), 2);
    }

    #[test]
    fn load_rejects_invalid_modal_spell_definitions() {
        let invalid = [
            r#"(
                id: "bad_bounds",
                name: "Bad Bounds",
                types: ["Instant"],
                modal_spell: (
                    min_modes: 2,
                    max_modes: 1,
                    modes: [(label: "Draw", effects: [Draw(count: 1)])],
                ),
            )"#,
            r#"(
                id: "empty_label",
                name: "Empty Label",
                types: ["Instant"],
                modal_spell: (
                    min_modes: 1,
                    max_modes: 1,
                    modes: [(label: " ", effects: [Draw(count: 1)])],
                ),
            )"#,
            r#"(
                id: "empty_effects",
                name: "Empty Effects",
                types: ["Instant"],
                modal_spell: (
                    min_modes: 1,
                    max_modes: 1,
                    modes: [(label: "Nothing", effects: [])],
                ),
            )"#,
        ];
        for bad in invalid {
            assert!(
                matches!(
                    CardRegistry::from_chunks(&[bad]),
                    Err(RegistryError::InvalidCard { .. })
                ),
                "expected invalid modal definition to be rejected"
            );
        }
    }

    #[test]
    fn load_rejects_modal_spell_with_another_resolution_owner() {
        let bad = r#"(
            id: "modal_double_owner",
            name: "Modal Double Owner",
            types: ["Instant"],
            spell_effect: [Draw(count: 1)],
            modal_spell: (
                min_modes: 1,
                max_modes: 1,
                modes: [(label: "Gain life", effects: [GainLife(amount: 3)])],
            ),
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(
            matches!(err, RegistryError::InvalidCard { ref reason, .. } if reason.contains("one resolution owner"))
        );
    }

    #[test]
    fn load_rejects_malformed_adventure_faces() {
        let invalid = [
            r#"(
                id: "one_face_adventure",
                name: "One Face Adventure",
                layout: Adventure,
                faces: [(name: "Creature", mana_cost: "{2}{G}", types: ["Creature"])],
            )"#,
            r#"(
                id: "spell_first_adventure",
                name: "Spell First Adventure",
                layout: Adventure,
                faces: [
                    (name: "Spell", mana_cost: "{G}", types: ["Instant"]),
                    (name: "Other Spell", mana_cost: "{1}{G}", types: ["Sorcery"]),
                ],
            )"#,
            r#"(
                id: "permanent_second_adventure",
                name: "Permanent Second Adventure",
                layout: Adventure,
                faces: [
                    (name: "Creature", mana_cost: "{2}{G}", types: ["Creature"]),
                    (name: "Other Creature", mana_cost: "{1}{G}", types: ["Creature"]),
                ],
            )"#,
        ];
        for bad in invalid {
            assert!(
                matches!(
                    CardRegistry::from_chunks(&[bad]),
                    Err(RegistryError::InvalidCard { .. })
                ),
                "expected malformed Adventure definition to be rejected"
            );
        }
    }

    #[test]
    fn load_rejects_malformed_attachment_definitions() {
        let invalid = [
            r#"(
                id: "aura_without_enchant",
                name: "Aura Without Enchant",
                mana_cost: "{W}",
                types: ["Enchantment", "Aura"],
                static_abilities: [AttachedModifier(keywords: [Flying])],
            )"#,
            r#"(
                id: "instant_aura_attach",
                name: "Instant Aura Attach",
                mana_cost: "{W}",
                types: ["Instant"],
                spell_effect: [AuraAttach(target: (kind: Creature))],
            )"#,
            r#"(
                id: "ordinary_enchantment_modifier",
                name: "Ordinary Enchantment Modifier",
                mana_cost: "{W}",
                types: ["Enchantment"],
                static_abilities: [AttachedModifier(keywords: [Flying])],
            )"#,
            r#"(
                id: "empty_attachment_modifier",
                name: "Empty Attachment Modifier",
                mana_cost: "{1}",
                types: ["Artifact", "Equipment"],
                static_abilities: [AttachedModifier()],
            )"#,
        ];
        for bad in invalid {
            assert!(
                matches!(
                    CardRegistry::from_chunks(&[bad]),
                    Err(RegistryError::InvalidCard { .. })
                ),
                "expected malformed attachment definition to be rejected"
            );
        }
    }

    #[test]
    fn content_hash_is_stable_and_well_formed() {
        let h = CardRegistry::content_hash();
        assert_eq!(h.len(), 16, "expected 16-char hex digest");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        // Deterministic within a build (same embedded data → same digest).
        assert_eq!(h, CardRegistry::content_hash());
    }
}
