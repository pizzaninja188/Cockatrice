use crate::card_def::CardDefinition;
use crate::primitives::EffectContext;
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
}

/// Name-index key normalization, applied to both stored names and lookup queries.
fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}

impl CardRegistry {
    pub fn from_embedded() -> Result<Self, RegistryError> {
        Self::from_chunks(EMBEDDED_RON_CHUNKS)
    }

    fn from_chunks(chunks: &[&str]) -> Result<Self, RegistryError> {
        let mut reg = CardRegistry::default();
        for chunk in chunks {
            let mut card: CardDefinition = RON_OPTS.from_str(chunk)?;
            // Type flags are derived from `types`/`supertypes`, not authored in RON.
            card.derive_type_flags();
            // Validate spell effects at startup. Spells have no source permanent, so `Self_`
            // target filters are rejected here (EffectContext::Spell).
            for effect in &card.spell_effect {
                effect.validate(EffectContext::Spell).map_err(|reason| {
                    RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    }
                })?;
            }
            // Validate activated ability effects (bound to a source permanent → Ability context).
            for aa in &card.activated_abilities {
                aa.effect
                    .validate(EffectContext::Ability)
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
            // Validate triggered ability effects (also Ability context — `Self_` is allowed).
            for ta in &card.triggered_abilities {
                ta.effect
                    .validate(EffectContext::Ability)
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
            let id = card.id.clone();
            if reg
                .by_name
                .insert(normalize_name(&card.name), id.clone())
                .is_some()
            {
                return Err(RegistryError::InvalidCard {
                    id,
                    reason: format!("duplicate name '{}'", card.name),
                });
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

    pub fn get(&self, id: &str) -> Option<&CardDefinition> {
        self.by_id.get(id)
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
        for chunk in EMBEDDED_RON_CHUNKS {
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
    use crate::primitives::{SpellEffectKind, TargetFilter, TargetKind};

    #[test]
    fn embedded_registry_loads() {
        CardRegistry::from_embedded().unwrap();
    }

    #[test]
    fn spell_effects_deserialize_from_ron() {
        let reg = CardRegistry::from_embedded().unwrap();
        assert_eq!(
            reg.get("angels_mercy").unwrap().spell_effect,
            vec![SpellEffectKind::GainLife { amount: 7 }]
        );
        assert_eq!(
            reg.get("lightning_bolt").unwrap().spell_effect,
            vec![SpellEffectKind::DamageTarget {
                amount: 3,
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
    fn content_hash_is_stable_and_well_formed() {
        let h = CardRegistry::content_hash();
        assert_eq!(h.len(), 16, "expected 16-char hex digest");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        // Deterministic within a build (same embedded data → same digest).
        assert_eq!(h, CardRegistry::content_hash());
    }
}
