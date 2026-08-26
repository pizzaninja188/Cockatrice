//! Card definitions, data-driven registry, and effect primitives.

pub mod card_def;
pub mod mana;
pub mod primitives;
pub mod registry;
pub mod slug;
pub mod token_def;

pub use card_def::{
    is_creature_type, CardDefinition, CardFace, CharacteristicDefiningAbility, FaceRef, Layout,
    ModalDef, ModeDef,
};
pub use mana::{ColorPip, ManaCost, ManaSymbol};
pub use primitives::{
    AbilityCost, AbilitySourceZone, ActivatedAbilityDef, ActivationCondition, ActivationTiming,
    AdditionalCost, Amount, BattlefieldAggregate, BattlefieldCreatureCountFilter,
    BattlefieldPermanentFilter, CardResultAction, CardResultFilter, CardResultSource,
    CardSearchZone, CastCostConditionalAmount, CastCostGroupDef, CastCostOptionDef,
    CastCostReceiptCondition, CastTriggerPlayer, Color, CombatRole, ConditionalManaOutput,
    ConditionalSearchDestination, ContinuousEffectKind, ControllerReference, CountExpression,
    CounterKind, DelayedTokenSacrificeTiming, EffectContext, EffectDuration, Evasion,
    GameCondition, GraveyardAggregate, Keyword, LibraryPartitionKind, ManaAmount, ManaSpendFilter,
    ManaSpendingRestriction, PermanentTypeFilter, PowerComparison, PtScale, RelativePlayerSet,
    SearchDestination, SearchZoneSelection, SpellCostModifier, SpellEffectKind, TriggerCondition,
    TriggeredAbilityDef, TypeLineAddition, ZoneCardFilter,
};
pub use registry::CardRegistry;
pub use slug::slugify;
pub use token_def::TokenDefinition;
