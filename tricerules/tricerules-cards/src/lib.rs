//! Card definitions, data-driven registry, and effect primitives.

pub mod card_def;
pub mod identity;
pub mod mana;
pub mod presentation;
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
    IdentifiedAbility, ModeId, SearchResultId,
};
pub use mana::{ColorPip, ManaCost, ManaSymbol};
pub use presentation::PresentationFaceMetadata;
pub use primitives::{
    AbilityCost, AbilitySourceZone, ActivatedAbilityDef, ActivationTiming, AdditionalCost, Amount,
    BasePowerToughnessValue, BattlefieldAggregate, BattlefieldCreatureCountFilter,
    BattlefieldPermanentFilter, CardResultAction, CardResultFilter, CardResultSource,
    CardSearchZone, CastCostConditionalAmount, CastCostGroupDef, CastCostOptionDef,
    CastCostOptionRef, CastCostReceiptCondition, CastOrdinalScope, CastTriggerPlayer, Color,
    CombatRole, ConditionalManaOutput, ConditionalSearchDestination, ContinuousEffectKind,
    ControllerReference, CountExpression, CounterKind, CounterRemovalPaymentSource,
    CreatureTypeChange, DelayedTokenSacrificeTiming, EffectContext, EffectDuration, Evasion,
    GameCondition, GraveyardAggregate, Keyword, LibraryPartitionKind, ManaAmount, ManaSpendFilter,
    ManaSpendingRestriction, ObjectCastCostKind, ObjectContributionKind, ObjectPaymentConstraint,
    PermanentChoiceConstraint, PermanentTypeFilter, PowerComparison, PowerToughnessCharacteristic,
    PtScale, PtScaleBasis, QuantityTerm, RelativePlayerSet, SearchDestination, SearchZoneSelection,
    SpecialActionManaPurpose, SpellCastFilter, SpellCastOrigin, SpellCostModifier, SpellEffectKind,
    TargetMatchFilter, TargetObjectExclusion, TriggerCondition, TriggeredAbilityDef,
    TypeLineAddition, TypeLineReplacement, ZoneCardFilter,
};
pub use registry::CardRegistry;
pub use slug::slugify;
pub use token_def::TokenDefinition;
