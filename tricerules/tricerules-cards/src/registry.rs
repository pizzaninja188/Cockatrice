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

static GLOBAL: Lazy<RwLock<CardRegistry>> = Lazy::new(|| {
    RwLock::new(CardRegistry::from_embedded().expect("embedded card data"))
});

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("ron parse: {0}")]
    Ron(#[from] ron::error::SpannedError),
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
            reg.by_id.insert(card.id.clone(), card);
        }
        Ok(reg)
    }

    pub fn get(&self, id: &str) -> Option<&CardDefinition> {
        self.by_id.get(id)
    }

    pub fn global() -> &'static RwLock<CardRegistry> {
        &GLOBAL
    }
}

/// Ron snippets compiled into the binary (hybrid model: data-first).
const EMBEDDED_RON_CHUNKS: &[&str] = &[
    include_str!("../data/mountain.ron"),
    include_str!("../data/island.ron"),
    include_str!("../data/forest.ron"),
    include_str!("../data/grizzly_bears.ron"),
    include_str!("../data/lightning_bolt.ron"),
    include_str!("../data/giant_growth.ron"),
    include_str!("../data/divination.ron"),
    include_str!("../data/go_for_the_throat.ron"),
    include_str!("../data/counterspell.ron"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_loads() {
        CardRegistry::from_embedded().unwrap();
    }
}
