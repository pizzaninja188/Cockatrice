//! Authoritative MTG-style game engine (vanilla core milestone).

pub mod custom;
pub mod engine;
pub mod state;

pub use engine::{Characteristics, EngineError, GameEngine};
pub use state::{
    AffectedScope, ContinuousEffect, GameObject, GameState, ObjectId, OpeningSequence, PlayerId,
    TurnStep, Zone,
};
