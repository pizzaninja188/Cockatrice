//! Card definitions, data-driven registry, and effect primitives.

pub mod card_def;
pub mod mana;
pub mod primitives;
pub mod registry;
pub mod slug;

pub use card_def::CardDefinition;
pub use mana::{ManaCost, ManaSymbol};
pub use primitives::{
    AbilityCost, ActivatedAbilityDef, CastTriggerPlayer, Color, ContinuousEffectKind,
    EffectContext, EffectDuration, Keyword, PermanentTypeFilter, TriggerCondition,
    TriggeredAbilityDef,
};
pub use registry::CardRegistry;
pub use slug::slugify;
