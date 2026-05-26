//! Card definitions, data-driven registry, and effect primitives.

pub mod card_def;
pub mod primitives;
pub mod registry;

pub use card_def::CardDefinition;
pub use primitives::{Color, Keyword};
pub use registry::CardRegistry;
