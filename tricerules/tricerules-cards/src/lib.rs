//! Embedded card data and offline authoring tools.
#[cfg(feature = "authoring")]
pub mod authoring;
pub mod authoring_schema;
pub mod registry;

// Data consumers use the same model types as the engine.
pub use tricerules_card_model::*;
pub use tricerules_card_model::{
    card_def, identity, mana, presentation, primitives, slug, token_def,
};

#[cfg(test)]
mod model_corpus_tests;
