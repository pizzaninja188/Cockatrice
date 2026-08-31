//! Card definitions, data-driven registry, and effect primitives.

pub mod card_def;
pub mod identity;
pub mod mana;
pub mod primitives;
pub mod registry;
pub mod slug;
pub mod token_def;

pub use card_def::{
    is_creature_type, CardDefinition, CardFace, CharacteristicDefiningAbility, FaceRef, Layout,
    ModalDef, ModeDef,
};
pub use identity::{
    ability_fallback, choice_fallback, external_oracle_lines, mode_fallback,
    resolve_external_presentation, AbilityId, AbilityPresentation, CardFaceId, ChoiceId,
    IdentifiedAbility, ModeId,
};
pub use mana::{ColorPip, ManaCost, ManaSymbol};
pub use primitives::{
    AbilityCost, AbilitySourceZone, ActivatedAbilityDef, ActivationCondition, ActivationTiming,
    AdditionalCost, Amount, BattlefieldAggregate, BattlefieldCreatureCountFilter,
    BattlefieldPermanentFilter, CardResultAction, CardResultFilter, CardResultSource,
    CardSearchZone, CastCostConditionalAmount, CastCostGroupDef, CastCostOptionDef,
    CastCostReceiptCondition, CastOrdinalScope, CastTriggerPlayer, Color, CombatRole,
    ConditionalManaOutput, ConditionalSearchDestination, ContinuousEffectKind, ControllerReference,
    CountExpression, CounterKind, DelayedTokenSacrificeTiming, EffectContext, EffectDuration,
    Evasion, GameCondition, GraveyardAggregate, Keyword, LibraryPartitionKind, ManaAmount,
    ManaSpendFilter, ManaSpendingRestriction, PermanentTypeFilter, PowerComparison,
    PowerToughnessCharacteristic, PtScale, QuantityTerm, RelativePlayerSet, SearchDestination,
    SearchZoneSelection, SpecialActionManaPurpose, SpellCastFilter, SpellCastOrigin,
    SpellCostModifier, SpellEffectKind, TargetMatchFilter, TargetObjectExclusion, TriggerCondition,
    TriggeredAbilityDef, TypeLineAddition, TypeLineReplacement, ZoneCardFilter,
};
pub use registry::CardRegistry;
pub use slug::slugify;
pub use token_def::TokenDefinition;
