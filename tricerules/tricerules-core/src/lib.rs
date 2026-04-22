//! Authoritative MTG-style game engine (vanilla core milestone).

pub mod engine;
pub mod state;

pub use engine::{EngineError, GameEngine};
pub use state::{GameObject, GamePhase, ObjectId, PlayerId, Zone};
