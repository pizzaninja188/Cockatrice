//! Stable card vocabulary, primitives, validation, and registry interface.

pub mod card_def;
pub mod identity;
pub mod mana;
pub mod presentation;
pub mod primitives;
pub mod registry;
pub mod slug;
pub mod token_def;

pub use card_def::{
    is_creature_type, CardDefinition, CardFace, CharacteristicDefiningAbility, ClassLevelBar,
    FaceRef, IdentifiedStaticAbility, Layout, ModalDef, ModeDef, PrototypeCharacteristics,
    TriggerModeSelectionRestriction, CREATURE_TYPES,
};
pub use identity::{
    ability_fallback, choice_fallback, external_oracle_lines, mode_fallback,
    resolve_external_presentation, AbilityDefinitionId, AbilityId, AbilityLinkId,
    AbilityPresentation, CardFaceDefinitionId, CardFaceId, ChoiceId, ExiledCohortId,
    IdentifiedAbility, ModeId, ProgramOwnerId, SearchResultId,
};
pub use mana::{ColorPip, ManaCost, ManaSymbol};
pub use presentation::PresentationFaceMetadata;
pub use primitives::{
    AbilityCost, AbilitySourceZone, ActivatedAbilityDef, ActivationTiming, AdditionalCost, Amount,
    AttackGroupDestination, AttackGroupTriggerTiming, AuthoredBlock, BasePowerToughnessValue,
    BasicLandType, BattlefieldAggregate, BattlefieldCreatureCountFilter,
    BattlefieldPermanentFilter, BattlefieldQuantityCharacteristic, CardProgram, CardResultAction,
    CardResultFilter, CardResultSource, CardSearchZone, Cardinality, CastCostConditionalAmount,
    CastCostGroupDef, CastCostOptionDef, CastCostOptionRef, CastCostReceiptCondition,
    CastOrdinalScope, CastTriggerPlayer, Color, CombatDamageTriggerCardinality, CombatRole,
    CompiledBlock, CompiledInstruction, CompiledInstructionKind, CompiledProgram,
    CompiledProgramCondition, CompiledProgramResultCondition, CompiledSlot, ConditionalManaOutput,
    ConditionalSearchDestination, ContinuousEffectKind, ControllerReference, CountExpression,
    CounterKind, CounterRemovalPaymentSource, CreatureTypeChange, DelayedTokenSacrificeTiming,
    DiscardResultOutcome, EffectContext, EffectDuration, EffectOp, Evasion, GainControlDuration,
    GameCondition, GraveyardAggregate, Instruction, InstructionKind, Keyword, LibraryPartitionKind,
    ManaAmount, ManaSpendFilter, ManaSpendingEffect, ManaSpendingRestriction, MassPlayerSet,
    ObjectCastCostKind, ObjectContributionKind, ObjectPaymentConstraint, PermanentChoiceConstraint,
    PermanentTypeFilter, PlayerComparisonMetric, PowerComparison, PowerToughnessCharacteristic,
    ProgramCondition, ProgramRequiredInput, ProgramResultCondition, ProgramTargetBinding,
    ProgramTargetProjection, ProgramTargetReference, ProgramTargetRole, PtScale, PtScaleBasis,
    QuantityTerm, RelativePlayerSet, ResolvingEffectDuration, ResolvingPermanentModifier,
    ResultSlotId, ResultType, SearchDestination, SearchZoneSelection, SourceEffectPath,
    SourceEffectPathStep, SpecialActionManaPurpose, SpellCastFilter, SpellCastOrigin,
    SpellCostModifier, SpellEffectKind, SpellKeyword, StaticEmblemEffect, TargetMatchFilter,
    TargetObjectExclusion, TriggerCondition, TriggeredAbilityDef, TypeLineAddition,
    TypeLineReplacement, ZoneCardFilter,
};
pub use registry::CardRegistry;
pub use slug::slugify;
pub use token_def::TokenDefinition;
