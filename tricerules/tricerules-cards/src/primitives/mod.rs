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
mod targeting;

pub use abilities::*;
pub use amounts::*;
pub use conditions::*;
pub use costs::*;
pub use effects::*;
pub use keywords::*;
pub use targeting::*;

#[cfg(test)]
mod tests;
