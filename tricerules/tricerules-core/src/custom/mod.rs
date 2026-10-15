//! Shared protocol choice vocabulary used by rules-engine choice producers.
//!
//! Card-specific continuation code has been retired in favor of typed data programs and engine
//! primitives. The card model still parses legacy `custom_effect` syntax to identify its key, and
//! registry admission rejects it before the engine can accept that card definition.

/// The protocol-owned choice kind, re-exported for existing engine call sites.
pub use tricerules_proto::ruled::v1::ChoiceKind;
