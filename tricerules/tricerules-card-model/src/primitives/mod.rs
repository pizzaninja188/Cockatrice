//! Generic data-driven vocabulary used by card definitions.
//!
//! The submodules group effects, targeting, costs, abilities, and keywords while the
//! re-exports below preserve the established `primitives::X` API and RON serde shapes.

mod abilities;
mod amounts;
mod conditions;
mod costs;
mod effects;
mod keywords;
mod presentation;
mod program;
mod targeting;

pub use abilities::*;
pub use amounts::*;
pub use conditions::*;
pub use costs::*;
pub use effects::*;
pub use keywords::*;
pub use program::*;
pub use targeting::*;
pub(crate) use targeting::{
    is_private_affected_player_discard_profile, program_effect_target_roles,
};

#[cfg(test)]
mod tests;
