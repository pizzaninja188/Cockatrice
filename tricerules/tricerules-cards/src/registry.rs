use crate::card_def::CardDefinition;
use crate::primitives::{EffectContext, SpellEffectKind};
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
            let mut card: CardDefinition = RON_OPTS.from_str(chunk)?;
            // Type flags are derived from `types`/`supertypes`, not authored in RON (per face).
            card.derive_type_flags();
            // Validate every face's effects at startup. `face(0)` of a Normal card is the flat
            // fields; multi-face cards (CR 709/712/715) validate each face uniformly. Spell
            // effects have no source permanent, so `Self_` target filters are rejected here
            // (EffectContext::Spell); activated/triggered effects bind to a source (Ability).
            for face in card.faces_iter() {
                // One resolution owner per face (CR 608): a custom (tier-3) effect and a
                // data-driven `spell_effect` list cannot both resolve the same face. The
                // matching custom impl is validated to exist on the `tricerules-core` side
                // (it owns the `CardEffect` lookup; this crate has no engine access).
                if face.custom_effect.is_some() && !face.spell_effect.is_empty() {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "face has both spell_effect and custom_effect (one resolution \
                                 owner allowed)"
                            .into(),
                    });
                }
                for effect in face.spell_effect {
                    effect.validate(EffectContext::Spell).map_err(|reason| {
                        RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        }
                    })?;
                }
                for aa in face.activated_abilities {
                    aa.effect
                        .validate(EffectContext::Ability)
                        .map_err(|reason| RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        })?;
                }
                for ta in face.triggered_abilities {
                    ta.effect
                        .validate(EffectContext::Ability)
                        .map_err(|reason| RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        })?;
                }
                // Every CreateTokens effect must name a loaded token (an uncreatable id is a bug).
                let all_effects = face
                    .spell_effect
                    .iter()
                    .chain(face.activated_abilities.iter().map(|a| &a.effect))
                    .chain(face.triggered_abilities.iter().map(|t| &t.effect));
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
            reg.get("angels_mercy").unwrap().spell_effect,
            vec![SpellEffectKind::GainLife {
                amount: Amount::Fixed(7)
            }]
        );
        assert_eq!(
            reg.get("lightning_bolt").unwrap().spell_effect,
            vec![SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(3),
                target: TargetFilter {
                    kind: TargetKind::AnyTarget,
                    not_artifact: false,
                    tapped: None,
                },
            }]
        );
        assert_eq!(
            reg.get("mind_sculpt").unwrap().spell_effect,
            vec![SpellEffectKind::MillTargetPlayer {
                count: 7,
                target: TargetFilter {
                    kind: TargetKind::OpponentPlayer,
                    not_artifact: false,
                    tapped: None,
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
        let card: CardDefinition = RON_OPTS.from_str(bad).unwrap();
        assert!(card.spell_effect[0]
            .validate(crate::primitives::EffectContext::Spell)
            .is_err());
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
                    effect: TargetPlayerGainsLife(amount: 3, target: (kind: Creature)),
                    text: "bad",
                ),
            ],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(matches!(err, RegistryError::InvalidCard { ref id, .. } if id == "bad_trigger"));
    }

    /// A self-pump trigger (the replacement for the old `TriggeredEffect::PumpSelf`) loads,
    /// while the same `Self_` filter in a spell's effect list is rejected at load.
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
                    trigger: AtBeginningOfControllerUpkeep,
                    effect: PumpTarget(power: 1, toughness: 1, target: (kind: Self_)),
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
            spell_effect: [PumpTarget(power: 1, toughness: 1, target: (kind: Self_))],
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
        assert!(soldier.is_creature);
        assert_eq!((soldier.power, soldier.toughness), (Some(1), Some(1)));
        // CR 111.4: color comes from the creating effect, not a (nonexistent) mana cost.
        assert_eq!(soldier.colors(), vec![Color::White]);
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
    fn content_hash_is_stable_and_well_formed() {
        let h = CardRegistry::content_hash();
        assert_eq!(h.len(), 16, "expected 16-char hex digest");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        // Deterministic within a build (same embedded data → same digest).
        assert_eq!(h, CardRegistry::content_hash());
    }
}
