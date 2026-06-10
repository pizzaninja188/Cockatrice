//! Card definitions, data-driven registry, and effect primitives.

pub mod card_def;
pub mod primitives;
pub mod registry;
pub mod slug;

pub use card_def::CardDefinition;
pub use primitives::{
    AbilityCost, ActivatedAbilityDef, Color, ContinuousEffectKind, EffectDuration, Keyword,
    TriggerCondition, TriggeredAbilityDef, TriggeredEffect,
};
pub use registry::CardRegistry;
pub use slug::slugify;
