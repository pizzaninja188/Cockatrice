use crate::card_def::CardDefinition;
use once_cell::sync::Lazy;
use ron::extensions::Extensions;
use ron::Options;
use std::collections::HashMap;
use std::sync::RwLock;
use thiserror::Error;

/// `Option` fields need `IMPLICIT_SOME` so bare values (e.g. `2` for `Option<u32>`) deserialize.
static RON_OPTS: Lazy<Options> =
    Lazy::new(|| Options::default().with_default_extension(Extensions::IMPLICIT_SOME));

static GLOBAL: Lazy<RwLock<CardRegistry>> =
    Lazy::new(|| RwLock::new(CardRegistry::from_embedded().expect("embedded card data")));

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
}

impl CardRegistry {
    pub fn from_embedded() -> Result<Self, RegistryError> {
        let mut reg = CardRegistry::default();
        for chunk in EMBEDDED_RON_CHUNKS {
            let card: CardDefinition = RON_OPTS.from_str(chunk)?;
            // Validate spell effect target-spec compatibility at startup.
            if let Some(effect) = &card.spell_effect {
                effect
                    .validate()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
            // Validate activated ability effects.
            for aa in &card.activated_abilities {
                aa.effect
                    .validate()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
            reg.by_id.insert(card.id.clone(), card);
        }
        Ok(reg)
    }

    pub fn get(&self, id: &str) -> Option<&CardDefinition> {
        self.by_id.get(id)
    }

    /// Iterate over every loaded card definition (order is unspecified).
    pub fn definitions(&self) -> impl Iterator<Item = &CardDefinition> {
        self.by_id.values()
    }

    pub fn global() -> &'static RwLock<CardRegistry> {
        &GLOBAL
    }
}

/// Ron snippets compiled into the binary (hybrid model: data-first).
const EMBEDDED_RON_CHUNKS: &[&str] = &[
    include_str!("../data/plains.ron"),
    include_str!("../data/mountain.ron"),
    include_str!("../data/island.ron"),
    include_str!("../data/forest.ron"),
    include_str!("../data/swamp.ron"),
    include_str!("../data/grizzly_bears.ron"),
    include_str!("../data/savannah_lions.ron"),
    include_str!("../data/walking_corpse.ron"),
    include_str!("../data/balduvian_barbarians.ron"),
    include_str!("../data/coral_merfolk.ron"),
    include_str!("../data/lightning_bolt.ron"),
    include_str!("../data/giant_growth.ron"),
    include_str!("../data/divination.ron"),
    include_str!("../data/go_for_the_throat.ron"),
    include_str!("../data/counterspell.ron"),
    include_str!("../data/healing_salve.ron"),
    include_str!("../data/angels_mercy.ron"),
    include_str!("../data/bump_in_the_night.ron"),
    include_str!("../data/blood_tithe.ron"),
    include_str!("../data/swords_to_plowshares.ron"),
    include_str!("../data/eyeblights_ending.ron"),
    include_str!("../data/unsummon.ron"),
    include_str!("../data/boomerang.ron"),
    include_str!("../data/tome_scour.ron"),
    include_str!("../data/mind_sculpt.ron"),
    include_str!("../data/storm_crow.ron"),
    include_str!("../data/giant_spider.ron"),
    include_str!("../data/accursed_spirit.ron"),
    include_str!("../data/ornithopter.ron"),
    include_str!("../data/alpine_watchdog.ron"),
    include_str!("../data/child_of_night.ron"),
    include_str!("../data/raging_goblin.ron"),
    include_str!("../data/pharikas_chosen.ron"),
    include_str!("../data/goblin_trailblazer.ron"),
    include_str!("../data/colossal_dreadmaw.ron"),
    include_str!("../data/goblin_striker.ron"),
    include_str!("../data/fencing_ace.ron"),
    // Activated ability cards
    include_str!("../data/prodigal_sorcerer.ron"),
    include_str!("../data/prodigal_pyromancer.ron"),
    include_str!("../data/royal_assassin.ron"),
    include_str!("../data/jayemdae_tome.ron"),
    include_str!("../data/icy_manipulator.ron"),
    include_str!("../data/bottle_gnomes.ron"),
    // Triggered ability cards
    include_str!("../data/elvish_visionary.ron"),
    include_str!("../data/flametongue_kavu.ron"),
    include_str!("../data/scroll_thief.ron"),
    include_str!("../data/thieving_magpie.ron"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::{SpellEffectKind, TargetSpec};

    #[test]
    fn embedded_registry_loads() {
        CardRegistry::from_embedded().unwrap();
    }

    #[test]
    fn spell_effects_deserialize_from_ron() {
        let reg = CardRegistry::from_embedded().unwrap();
        assert_eq!(
            reg.get("angels_mercy").unwrap().spell_effect,
            Some(SpellEffectKind::GainLife { amount: 7 })
        );
        assert_eq!(
            reg.get("lightning_bolt").unwrap().spell_effect,
            Some(SpellEffectKind::DamageTarget {
                amount: 3,
                target: TargetSpec::AnyTarget,
            })
        );
        assert_eq!(
            reg.get("mind_sculpt").unwrap().spell_effect,
            Some(SpellEffectKind::MillTargetPlayer {
                count: 7,
                target: TargetSpec::OpponentPlayer,
            })
        );
    }

    #[test]
    fn startup_validation_rejects_incompatible_target_spec() {
        // A player-life effect pointed at a creature is invalid card data.
        let bad = r#"(
            id: "bad_card",
            name: "Bad Card",
            mana_cost: "W",
            types: ["Instant"],
            is_instant: true,
            spell_effect: TargetPlayerGainsLife(amount: 3, target: Creature),
        )"#;
        let card: CardDefinition = RON_OPTS.from_str(bad).unwrap();
        assert!(card.spell_effect.unwrap().validate().is_err());
    }
}
