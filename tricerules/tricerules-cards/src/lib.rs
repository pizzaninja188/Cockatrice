//! Card definitions, data-driven registry, and effect primitives.

pub mod card_def;
pub mod mana;
pub mod primitives;
pub mod registry;
pub mod slug;
pub mod token_def;

pub use card_def::{CardDefinition, CardFace, FaceRef, Layout, ModalDef, ModeDef};
pub use mana::{ColorPip, ManaCost, ManaSymbol};
pub use primitives::{
    AbilityCost, ActivatedAbilityDef, ActivationCondition, ActivationTiming, AdditionalCost,
    Amount, BattlefieldAggregate, BattlefieldCreatureCountFilter, BattlefieldPermanentFilter,
    CastTriggerPlayer, Color, ConditionalManaOutput, ContinuousEffectKind, ControllerReference,
    CountExpression, CounterKind, EffectContext, EffectDuration, Evasion, GameCondition,
    GraveyardAggregate, Keyword, ManaAmount, ManaSpendFilter, ManaSpendingRestriction,
    PermanentTypeFilter, PtScale, RelativePlayerSet, SpellCostModifier, TriggerCondition,
    TriggeredAbilityDef, TypeLineAddition,
};
pub use registry::CardRegistry;
pub use slug::slugify;
pub use token_def::TokenDefinition;
