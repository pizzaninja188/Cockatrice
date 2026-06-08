//! Authoritative MTG-style game engine (vanilla core milestone).

pub mod engine;
pub mod state;

pub use engine::{EngineError, GameEngine};
pub use state::{
    AffectedScope, ContinuousEffect, GameObject, GameState, ObjectId, OpeningSequence, PlayerId,
    TurnStep, Zone,
};
