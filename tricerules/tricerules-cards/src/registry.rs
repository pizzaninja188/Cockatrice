use crate::card_def::{CardDefinition, CardFace, Layout, RawCardDefinition};
use crate::primitives::{
    AbilityCost, ActivatedCostModifier, AdditionalCost, Amount, BattlefieldAggregate,
    CardResultAction, CardResultSource, CastCostGroupDef, CastCostReceiptCondition, EffectContext,
    FaceChangeAction, GameCondition, ObjectContributionKind, ResolutionBranchRequirement,
    SpecialActionAffected, SpellEffectKind, StaticAbilityDef, TargetController, TargetKind,
    TargetingDef, TriggerCondition, ZoneCardFilter,
};
use crate::token_def::TokenDefinition;
use crate::ManaSymbol;
use crate::PresentationFaceMetadata;
use once_cell::sync::Lazy;
use ron::extensions::Extensions;
use ron::Options;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// `Option` fields need `IMPLICIT_SOME` so bare values (e.g. `2` for `Option<u32>`) deserialize.
static RON_OPTS: Lazy<Options> =
    Lazy::new(|| Options::default().with_default_extension(Extensions::IMPLICIT_SOME));

/// Parsed once per process and shared by every game (read-only after init).
/// Panics on invalid embedded data: fail-fast at sidecar startup is the validation point.
static GLOBAL: Lazy<CardRegistry> =
    Lazy::new(|| CardRegistry::from_embedded().expect("embedded card data"));

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("ron parse: {0}")]
    Ron(#[from] ron::error::SpannedError),
    #[error("invalid card data for '{id}': {reason}")]
    InvalidCard { id: String, reason: String },
}

#[derive(Debug, Default)]
pub struct CardRegistry {
    by_id: HashMap<String, CardDefinition>,
    /// Trimmed, lowercased Oracle name -> card id (see [`Self::id_for_name`]).
    by_name: HashMap<String, String>,
    /// Token namespace: token id -> the [`CardDefinition`] synthesized from its
    /// [`TokenDefinition`] (CR 111). Kept apart from `by_id` so tokens are never deck cards
    /// or counted as implemented Oracle cards, but [`Self::get`] falls back here so the engine's
    /// characteristic queries work uniformly for token objects.
    tokens: HashMap<String, CardDefinition>,
    presentation_faces: HashMap<(String, String), PresentationFaceMetadata>,
}

/// Name-index key normalization, applied to both stored names and lookup queries.
fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}

fn face_can_reference_attached_object(face: &CardFace) -> bool {
    if face.types.iter().any(|card_type| card_type == "Equipment") {
        return true;
    }
    face.is_aura
        && face.spell_effect.iter().any(|effect| {
            matches!(
                effect,
                SpellEffectKind::AuraAttach { target } if !target.is_player()
            )
        })
}

fn validate_saga_face(card: &CardDefinition, face: &CardFace) -> Result<(), RegistryError> {
    let is_saga = face
        .types
        .iter()
        .any(|card_type| card_type == "Enchantment")
        && face.types.iter().any(|card_type| card_type == "Saga");
    let chapter_abilities: Vec<_> = face
        .triggered_abilities
        .iter()
        .filter(|ability| matches!(ability.trigger, TriggerCondition::SagaChapter { .. }))
        .collect();
    if !chapter_abilities.is_empty() && !is_saga {
        return Err(RegistryError::InvalidCard {
            id: card.id.clone(),
            reason: "Saga chapter triggers require an Enchantment Saga face".into(),
        });
    }
    if face
        .keywords
        .contains(&crate::primitives::Keyword::ReadAhead)
        && (!is_saga || chapter_abilities.is_empty())
    {
        return Err(RegistryError::InvalidCard {
            id: card.id.clone(),
            reason: "Read ahead requires an Enchantment Saga face with chapter abilities".into(),
        });
    }
    Ok(())
}

fn face_can_reference_attached_player(face: &CardFace) -> bool {
    face.is_aura
        && face.spell_effect.iter().any(
            |effect| matches!(effect, SpellEffectKind::AuraAttach { target } if target.is_player()),
        )
}

fn validate_cast_cost_condition(
    groups: &[CastCostGroupDef],
    condition: &CastCostReceiptCondition,
) -> Result<(), String> {
    let group = groups
        .iter()
        .find(|group| group.group_id == condition.group_id)
        .ok_or_else(|| "cast-cost condition references an unknown group".to_string())?;
    if !group
        .options
        .iter()
        .any(|option| option.option_id() == &condition.option_id)
    {
        return Err("cast-cost condition references an unknown option".into());
    }
    Ok(())
}

fn validate_effect_cast_cost_conditions(
    groups: &[CastCostGroupDef],
    effect: &SpellEffectKind,
) -> Result<(), String> {
    let amount = match effect {
        SpellEffectKind::DamageTarget { amount, .. }
        | SpellEffectKind::DamageAll { amount, .. }
        | SpellEffectKind::DamageTargets { amount, .. }
        | SpellEffectKind::DamagePlayer { amount, .. }
        | SpellEffectKind::DamageAttackedPlayerOrPlaneswalker { amount }
        | SpellEffectKind::Scry { count: amount }
        | SpellEffectKind::Earthbend { count: amount }
        | SpellEffectKind::CounterTargetSpell {
            unless_controller_pays: Some(amount),
            ..
        }
        | SpellEffectKind::Draw { count: amount, .. }
        | SpellEffectKind::GainLife { amount }
        | SpellEffectKind::Mill { count: amount, .. }
        | SpellEffectKind::PutCounters { count: amount, .. }
        | SpellEffectKind::Amass { count: amount, .. }
        | SpellEffectKind::CreateTokens { count: amount, .. }
        | SpellEffectKind::CreateTokenCopies { count: amount, .. }
        | SpellEffectKind::CreateAttackingTokens { count: amount, .. } => Some(amount),
        SpellEffectKind::PumpTarget {
            scale: Some(scale), ..
        } => scale.amount(),
        _ => None,
    };
    if let Some(value) = amount.and_then(Amount::cast_cost_amount) {
        validate_cast_cost_condition(groups, &value.condition)?;
    }
    match effect {
        SpellEffectKind::ConditionalCastCost { condition, effect } => {
            validate_cast_cost_condition(groups, condition)?;
            validate_effect_cast_cost_conditions(groups, effect)
        }
        SpellEffectKind::CounterTargetSpell {
            unless_controller_pays_by_cast_cost: Some(conditional),
            ..
        }
        | SpellEffectKind::SearchLibrary {
            count_by_cast_cost: Some(conditional),
            ..
        }
        | SpellEffectKind::ExileTopWithPlayPermission {
            count_by_cast_cost: Some(conditional),
            ..
        } => validate_cast_cost_condition(groups, &conditional.condition),
        SpellEffectKind::SearchLibrary { slots, .. } => {
            for condition in slots
                .iter()
                .filter_map(|slot| slot.enabled_by_cast_cost.as_ref())
            {
                validate_cast_cost_condition(groups, condition)?;
            }
            Ok(())
        }
        SpellEffectKind::ChooseResolutionBranch { branches, .. } => {
            for branch in branches {
                if let ResolutionBranchRequirement::CastCostReceipt(condition) = &branch.requirement
                {
                    validate_cast_cost_condition(groups, condition)?;
                }
                for nested in &branch.effects {
                    validate_effect_cast_cost_conditions(groups, nested)?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_effect_payment_results(
    allowed: &[CardResultAction],
    effect: &SpellEffectKind,
) -> Result<(), String> {
    let amount = match effect {
        SpellEffectKind::DamageTarget { amount, .. }
        | SpellEffectKind::DamageAll { amount, .. }
        | SpellEffectKind::Scry { count: amount }
        | SpellEffectKind::Earthbend { count: amount }
        | SpellEffectKind::CounterTargetSpell {
            unless_controller_pays: Some(amount),
            ..
        }
        | SpellEffectKind::DamagePlayer { amount, .. }
        | SpellEffectKind::Draw { count: amount, .. }
        | SpellEffectKind::GainLife { amount }
        | SpellEffectKind::Mill { count: amount, .. }
        | SpellEffectKind::PutCounters { count: amount, .. }
        | SpellEffectKind::Amass { count: amount, .. }
        | SpellEffectKind::CreateTokens { count: amount, .. }
        | SpellEffectKind::CreateTokenCopies { count: amount, .. }
        | SpellEffectKind::CreateAttackingTokens { count: amount, .. } => Some(amount),
        SpellEffectKind::PumpTarget {
            scale: Some(scale), ..
        } => scale.amount(),
        _ => None,
    };
    if let Some(filter) = amount.and_then(Amount::card_result_filter) {
        if filter.source == CardResultSource::Payment && !allowed.contains(&filter.action) {
            return Err("Payment card result requires a compatible card cost".into());
        }
    }
    if let SpellEffectKind::ChooseResolutionBranch { branches, .. } = effect {
        for branch in branches {
            if let ResolutionBranchRequirement::CardResultCount { filter, .. } = &branch.requirement
            {
                if filter.source == CardResultSource::Payment && !allowed.contains(&filter.action) {
                    return Err("Payment card result requires a compatible card cost".into());
                }
            }
            for nested in &branch.effects {
                validate_effect_payment_results(allowed, nested)?;
            }
        }
    }
    Ok(())
}

fn additional_cost_result_actions(costs: &[AdditionalCost]) -> Vec<CardResultAction> {
    costs
        .iter()
        .filter_map(|cost| match cost {
            AdditionalCost::DiscardCard => Some(CardResultAction::Discard),
            AdditionalCost::ExileGraveyardCards { .. } => Some(CardResultAction::Exile),
            AdditionalCost::SacrificePermanent { .. } => Some(CardResultAction::Sacrifice),
            AdditionalCost::TapPermanents { .. } => Some(CardResultAction::Tap),
            AdditionalCost::Blight { .. } => None,
        })
        .collect()
}

fn ability_cost_result_actions(costs: &[AbilityCost]) -> Vec<CardResultAction> {
    costs
        .iter()
        .filter_map(|cost| match cost {
            AbilityCost::Discard | AbilityCost::DiscardSelf => Some(CardResultAction::Discard),
            AbilityCost::ExileSelf | AbilityCost::ExileGraveyardCards { .. } => {
                Some(CardResultAction::Exile)
            }
            AbilityCost::SacrificeSelf | AbilityCost::SacrificePermanent { .. } => {
                Some(CardResultAction::Sacrifice)
            }
            AbilityCost::Tap
            | AbilityCost::Blight { .. }
            | AbilityCost::RemoveCounters { .. }
            | AbilityCost::TapPermanents { .. }
            | AbilityCost::Mana(_)
            | AbilityCost::Waterbend(_)
            | AbilityCost::Loyalty(_) => {
                matches!(cost, AbilityCost::TapPermanents { .. }).then_some(CardResultAction::Tap)
            }
        })
        .collect()
}

// Shared by deck cards and fixed tokens, so token abilities cannot bypass authoring checks.
fn validate_static_abilities(card: &CardDefinition, face: &CardFace) -> Result<(), RegistryError> {
    let attachment_source = face.is_aura || face.types.iter().any(|t| t == "Equipment");
    for identified in &face.static_abilities {
        identified
            .validate_metadata()
            .map_err(|reason| RegistryError::InvalidCard {
                id: card.id.clone(),
                reason,
            })?;
        let ability = &identified.definition;
        if let StaticAbilityDef::AdditionalTriggeredAbilityInstances {
            source_filter,
            condition,
            additional_count,
            ..
        } = ability
        {
            if *additional_count == 0 {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "AdditionalTriggeredAbilityInstances additional_count must be nonzero"
                        .into(),
                });
            }
            source_filter
                .validate()
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
            if let Some(condition) = condition {
                condition
                    .validate_live()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
        }
        if let StaticAbilityDef::SelfDoesntUntapDuringUntapStepUnless { condition } = ability {
            condition
                .validate_live()
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
        }
        if let StaticAbilityDef::GrantTriggeredAbilityToPermanents {
            filter,
            condition,
            triggered_abilities,
        } = ability
        {
            filter
                .validate_characteristic_constraints()
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
            if !filter.is_permanent_only() {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "GrantTriggeredAbilityToPermanents requires a permanent-only filter"
                        .into(),
                });
            }
            if triggered_abilities.is_empty() {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "GrantTriggeredAbilityToPermanents requires at least one ability"
                        .into(),
                });
            }
            if let Some(condition) = condition {
                condition
                    .validate_live()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
            for granted in triggered_abilities {
                if granted.trigger.is_delayed_only()
                    || matches!(granted.trigger, TriggerCondition::SagaChapter { .. })
                {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "GrantTriggeredAbilityToPermanents requires an ordinary non-Saga trigger"
                            .into(),
                    });
                }
                granted
                    .validate_shape()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
        }
        if let StaticAbilityDef::EntersTapped {
            condition: Some(condition),
            ..
        } = ability
        {
            condition
                .validate_live()
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
        }
        if let StaticAbilityDef::TargetingCostIncrease {
            protected, amount, ..
        } = ability
        {
            if *amount == 0 {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "TargetingCostIncrease amount must be nonzero".into(),
                });
            }
            if let crate::primitives::TargetingCostProtected::Creatures(filter) = protected {
                filter
                    .validate()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
        }
        if let StaticAbilityDef::AnthemPt {
            filter, condition, ..
        }
        | StaticAbilityDef::AnthemKeyword {
            filter, condition, ..
        } = ability
        {
            filter
                .validate()
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
            if let Some(condition) = condition {
                condition
                    .validate_live()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
                if condition.any_node_matches(|node| {
                    matches!(
                        node,
                        GameCondition::BattlefieldAggregate {
                            aggregate: BattlefieldAggregate::DistinctNames
                                | BattlefieldAggregate::TotalPower
                                | BattlefieldAggregate::MaximumPower,
                            ..
                        }
                    )
                }) {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "conditional layer-6/7 anthems support only simple battlefield counts until CR 613.8 dependency ordering is implemented".into(),
                    });
                }
                if condition.any_node_matches(|node| {
                    matches!(node, GameCondition::BattlefieldCreatureCount { .. })
                }) {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "conditional layer-6/7 anthems cannot depend on derived creature counts until CR 613.8 dependency ordering is implemented".into(),
                    });
                }
            }
        }
        if let StaticAbilityDef::SpellGenericReduction {
            amount, condition, ..
        } = ability
        {
            amount
                .validate_cost(true)
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
            if let Some(condition) = condition {
                condition
                    .validate_live()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
        }
        if let StaticAbilityDef::ConditionalSelfModifier {
            condition,
            add_types,
            base_power,
            base_toughness,
            delta_power,
            delta_toughness,
            keywords,
            activated_abilities,
            triggered_abilities,
            can_attack_as_though_without_defender,
        } = ability
        {
            condition
                .validate_live()
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
            if *delta_power == 0
                && *delta_toughness == 0
                && add_types.is_empty()
                && base_power.is_none()
                && base_toughness.is_none()
                && keywords.is_empty()
                && activated_abilities.is_empty()
                && triggered_abilities.is_empty()
                && !can_attack_as_though_without_defender
            {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "ConditionalSelfModifier must modify at least one value".into(),
                });
            }
            if base_power.is_some() != base_toughness.is_some() {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason:
                        "ConditionalSelfModifier base power and toughness must be provided together"
                            .into(),
                });
            }
            if !add_types.is_empty() {
                add_types
                    .validate()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
            for ability in activated_abilities {
                ability
                    .validate_shape()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
                let allowed = ability_cost_result_actions(&ability.costs);
                for effect in &ability.effect {
                    validate_effect_payment_results(&allowed, effect).map_err(|reason| {
                        RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        }
                    })?;
                }
            }
            for ability in triggered_abilities {
                ability
                    .validate_shape()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
            if (*delta_power != 0
                || *delta_toughness != 0
                || !add_types.is_empty()
                || base_power.is_some()
                || !keywords.is_empty()
                || !activated_abilities.is_empty())
                && condition.any_node_matches(|node| {
                    matches!(
                        node,
                        crate::primitives::GameCondition::BattlefieldAggregate {
                            aggregate: BattlefieldAggregate::DistinctNames
                                | BattlefieldAggregate::TotalPower
                                | BattlefieldAggregate::MaximumPower,
                            ..
                        }
                    )
                })
            {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "conditional layer-6/7 modifiers support only simple battlefield counts until CR 613.8 dependency ordering is implemented".into(),
                });
            }
            if condition.any_node_matches(|node| {
                matches!(node, GameCondition::BattlefieldCreatureCount { .. })
            }) {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "conditional self modifiers cannot depend on derived creature counts until CR 613.8 dependency ordering is implemented".into(),
                });
            }
        }
        if let StaticAbilityDef::CountScaledSelfPt {
            count,
            power_per_match,
            toughness_per_match,
        } = ability
        {
            count
                .validate_static_count()
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
            if *power_per_match == 0 && *toughness_per_match == 0 {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "CountScaledSelfPt must modify power or toughness".into(),
                });
            }
        }
        if let StaticAbilityDef::EntersAsCopy { filter } = ability {
            filter
                .validate_characteristic_constraints()
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
            if !filter.all_terminal_filters_match(|leaf| {
                matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                    && leaf.controller == TargetController::Any
                    && leaf.excluded_objects.is_empty()
            }) {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "EntersAsCopy requires an untargeted Creature or AnyPermanent filter"
                        .into(),
                });
            }
        }
        if let StaticAbilityDef::EntersWithCounters {
            affected,
            counter,
            amount,
            cast_cost_condition,
        } = ability
        {
            if let Some(condition) = cast_cost_condition {
                validate_cast_cost_condition(&face.cast_cost_groups, condition).map_err(
                    |reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    },
                )?;
            }
            counter
                .validate()
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
            if let crate::primitives::EntersWithCountersAffected::Creatures(filter) = affected {
                filter
                    .validate()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
            if amount.card_result_filter().is_some() {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "card result counts are valid only in a resolving effect list".into(),
                });
            }
            amount
                .validate_live()
                .and_then(|()| amount.validate_source_context(false))
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
        }
        if let StaticAbilityDef::PreventDamage {
            additional_effect:
                Some(crate::primitives::DamagePreventionAdditionalEffect::PutCounters {
                    counter, ..
                }),
            ..
        } = ability
        {
            counter
                .validate()
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
        }
        if let StaticAbilityDef::AttachedModifier {
            condition,
            add_types,
            set_types,
            set_name,
            set_colors,
            delta_power,
            delta_toughness,
            set_power,
            set_toughness,
            remove_all_abilities,
            keywords,
            triggered_abilities,
            activated_abilities,
            restriction,
            doesnt_untap_during_untap_step,
            cant_untap,
        } = ability
        {
            if !attachment_source {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "AttachedModifier requires an Aura or Equipment source".into(),
                });
            }
            if *delta_power == 0
                && *delta_toughness == 0
                && set_power.is_none()
                && set_toughness.is_none()
                && !remove_all_abilities
                && add_types.is_empty()
                && set_types.is_none()
                && set_name.is_none()
                && set_colors.is_none()
                && keywords.is_empty()
                && triggered_abilities.is_empty()
                && activated_abilities.is_empty()
                && restriction.is_empty()
                && !doesnt_untap_during_untap_step
                && !cant_untap
            {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "AttachedModifier must modify at least one value".into(),
                });
            }
            if set_power.is_some() != set_toughness.is_some() {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "AttachedModifier must set both power and toughness".into(),
                });
            }
            if !add_types.is_empty() && set_types.is_some() {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "AttachedModifier cannot both add and replace types".into(),
                });
            }
            if !add_types.is_empty() {
                add_types
                    .validate()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
            if let Some(replacement) = set_types {
                replacement
                    .validate()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
            if set_name.as_ref().is_some_and(|name| name.trim().is_empty()) {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "AttachedModifier set_name cannot be empty".into(),
                });
            }
            if let Some(colors) = set_colors {
                let unique: std::collections::HashSet<_> = colors.iter().copied().collect();
                if unique.len() != colors.len() {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "AttachedModifier set_colors repeats a color".into(),
                    });
                }
            }
            if let Some(condition) = condition {
                condition
                    .validate_live()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
                if !triggered_abilities.is_empty()
                    || !activated_abilities.is_empty()
                    || !restriction.is_empty()
                    || *doesnt_untap_during_untap_step
                    || *cant_untap
                {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason:
                            "conditioned AttachedModifier only supports characteristic modifiers"
                                .into(),
                    });
                }
                if (*delta_power != 0
                    || *delta_toughness != 0
                    || set_power.is_some()
                    || *remove_all_abilities
                    || !keywords.is_empty())
                    && condition.any_node_matches(|node| {
                        matches!(
                            node,
                            crate::primitives::GameCondition::BattlefieldAggregate {
                                aggregate: BattlefieldAggregate::TotalPower
                                    | BattlefieldAggregate::MaximumPower,
                                ..
                            }
                        )
                    })
                {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "power-dependent conditional characteristics require CR 613.8 dependency ordering"
                            .into(),
                    });
                }
            }
            if !restriction.is_empty() {
                restriction
                    .validate()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
            for granted in triggered_abilities {
                if granted.trigger.is_delayed_only() {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "AttachedModifier cannot grant a delayed trigger".into(),
                    });
                }
                granted
                    .validate_shape()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
            for granted in activated_abilities {
                granted
                    .validate_shape()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
        }
        if let StaticAbilityDef::ProhibitSpecialAction {
            affected,
            condition,
            ..
        } = ability
        {
            if matches!(affected, SpecialActionAffected::AttachedPermanent) && !attachment_source {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason:
                        "attached special-action prohibition requires an Aura or Equipment source"
                            .into(),
                });
            }
            if let SpecialActionAffected::Permanents(filter) = affected {
                if filter.any_terminal_filter_matches(|leaf| !leaf.excluded_objects.is_empty()) {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "special-action scopes do not bind object exclusions".into(),
                    });
                }
                filter
                    .validate_characteristic_constraints()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
            if let Some(condition) = condition {
                condition
                    .validate_live()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
        }
        if let StaticAbilityDef::SelfCombatRestriction {
            restriction,
            condition,
        } = ability
        {
            restriction
                .validate()
                .and_then(|()| {
                    condition
                        .as_ref()
                        .map_or(Ok(()), GameCondition::validate_live)
                })
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
        }
        if let StaticAbilityDef::CreatureScopeCombatRestriction {
            filter,
            restriction,
        } = ability
        {
            filter
                .validate()
                .and_then(|()| restriction.validate())
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
        }
        if matches!(ability, StaticAbilityDef::ControlsAttached) && !face.is_aura {
            return Err(RegistryError::InvalidCard {
                id: card.id.clone(),
                reason: "ControlsAttached requires an Aura source".into(),
            });
        }
    }
    Ok(())
}

fn insert_ability_id(ids: &mut HashSet<String>, id: &crate::AbilityId) -> Result<(), String> {
    id.validate()?;
    if !ids.insert(id.as_str().to_owned()) {
        return Err(format!("duplicate sibling ability id '{}'", id));
    }
    Ok(())
}

fn validate_nested_effect_metadata(effect: &SpellEffectKind) -> Result<(), String> {
    match effect {
        SpellEffectKind::CreateReflexiveTrigger { ability } => {
            ability.validate_shape()?;
            validate_effect_list_metadata(&ability.effect)
        }
        SpellEffectKind::GrantTriggeredAbility { ability, .. }
        | SpellEffectKind::CreateDelayedTrigger { ability, .. } => {
            ability.validate_shape()?;
            validate_effect_list_metadata(&ability.effect)
        }
        SpellEffectKind::ChooseResolutionBranch { branches, .. } => {
            for branch in branches {
                validate_effect_list_metadata(&branch.effects)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_effect_list_metadata(effects: &[SpellEffectKind]) -> Result<(), String> {
    for effect in effects {
        validate_nested_effect_metadata(effect)?;
    }
    Ok(())
}

fn fixed_source_reduction_cost(cost: &crate::ManaCost) -> bool {
    cost.pips.iter().all(|symbol| {
        matches!(
            symbol,
            ManaSymbol::W
                | ManaSymbol::U
                | ManaSymbol::B
                | ManaSymbol::R
                | ManaSymbol::G
                | ManaSymbol::C
                | ManaSymbol::Generic(_)
        )
    })
}

fn validate_source_mana_cost_reduction(
    face: &CardFace,
    ability: &crate::primitives::ActivatedAbilityDef,
) -> Result<(), String> {
    if !ability.cost_modifiers.iter().any(|modifier| {
        matches!(
            modifier,
            ActivatedCostModifier::ConditionalSourceManaCostReduction { .. }
        )
    }) {
        return Ok(());
    }
    if !fixed_source_reduction_cost(&face.mana_cost) {
        return Err(
            "source mana cost reduction requires a source cost containing only fixed mana symbols"
                .into(),
        );
    }
    let ability_mana_cost = ability.costs.iter().find_map(|cost| match cost {
        AbilityCost::Mana(cost) | AbilityCost::Waterbend(cost) => Some(cost),
        _ => None,
    });
    if ability_mana_cost.is_none_or(|cost| !fixed_source_reduction_cost(cost)) {
        return Err(
            "source mana cost reduction requires an ability cost containing only fixed mana symbols"
                .into(),
        );
    }
    Ok(())
}

fn validate_face_identity(face: &CardFace) -> Result<(), String> {
    face.face_id.validate()?;
    let mut siblings = HashSet::new();
    for ability in &face.activated_abilities {
        insert_ability_id(&mut siblings, &ability.ability_id)?;
        ability.validate_shape()?;
        validate_source_mana_cost_reduction(face, ability)?;
        validate_effect_list_metadata(&ability.effect)?;
    }
    for ability in &face.triggered_abilities {
        insert_ability_id(&mut siblings, &ability.ability_id)?;
        ability.validate_shape()?;
        validate_effect_list_metadata(&ability.effect)?;
    }
    for ability in &face.static_abilities {
        insert_ability_id(&mut siblings, &ability.ability_id)?;
        ability.validate_metadata()?;
        let mut nested = HashSet::new();
        match &ability.definition {
            StaticAbilityDef::AttachedModifier {
                activated_abilities,
                triggered_abilities,
                ..
            } => {
                for nested_ability in activated_abilities {
                    insert_ability_id(&mut nested, &nested_ability.ability_id)?;
                    nested_ability.validate_shape()?;
                    validate_effect_list_metadata(&nested_ability.effect)?;
                }
                for nested_ability in triggered_abilities {
                    insert_ability_id(&mut nested, &nested_ability.ability_id)?;
                    nested_ability.validate_shape()?;
                    validate_effect_list_metadata(&nested_ability.effect)?;
                }
            }
            StaticAbilityDef::ConditionalSelfModifier {
                activated_abilities,
                triggered_abilities,
                ..
            } => {
                for nested_ability in activated_abilities {
                    insert_ability_id(&mut nested, &nested_ability.ability_id)?;
                    nested_ability.validate_shape()?;
                    validate_effect_list_metadata(&nested_ability.effect)?;
                }
                for nested_ability in triggered_abilities {
                    insert_ability_id(&mut nested, &nested_ability.ability_id)?;
                    nested_ability.validate_shape()?;
                    validate_effect_list_metadata(&nested_ability.effect)?;
                }
            }
            StaticAbilityDef::GrantTriggeredAbilityToPermanents {
                triggered_abilities,
                ..
            } => {
                for nested_ability in triggered_abilities {
                    insert_ability_id(&mut nested, &nested_ability.ability_id)?;
                    nested_ability.validate_shape()?;
                    validate_effect_list_metadata(&nested_ability.effect)?;
                }
            }
            _ => {}
        }
    }
    for ability in &face.characteristic_defining_abilities {
        insert_ability_id(&mut siblings, &ability.ability_id)?;
        ability.validate_metadata()?;
    }
    let mut cast_cost_group_ids = HashSet::new();
    for group in &face.cast_cost_groups {
        group.validate()?;
        if !cast_cost_group_ids.insert(group.group_id.as_str()) {
            return Err(format!("duplicate cast cost group id '{}'", group.group_id));
        }
    }
    let mut linked_costs = HashSet::new();
    if let Some(modal) = &face.modal_spell {
        if let Some(link) = &modal.all_modes_cast_cost {
            link.validate()?;
            validate_cast_cost_condition(
                &face.cast_cost_groups,
                &CastCostReceiptCondition {
                    group_id: link.group_id.clone(),
                    option_id: link.option_id.clone(),
                    expected_selected: true,
                },
            )?;
            linked_costs.insert((link.group_id.as_str(), link.option_id.as_str()));
        }
        for mode in &modal.modes {
            let Some(link) = &mode.linked_cast_cost else {
                continue;
            };
            link.validate()?;
            let condition = CastCostReceiptCondition {
                group_id: link.group_id.clone(),
                option_id: link.option_id.clone(),
                expected_selected: true,
            };
            validate_cast_cost_condition(&face.cast_cost_groups, &condition)?;
            if !linked_costs.insert((link.group_id.as_str(), link.option_id.as_str())) {
                return Err(format!(
                    "cast-cost option '{}.{}' is linked more than once by modal rules",
                    link.group_id, link.option_id
                ));
            }
        }
    }
    for targeting in std::iter::once(face.targeting.as_ref())
        .chain(
            face.modal_spell
                .iter()
                .flat_map(|modal| modal.modes.iter().map(|mode| mode.targeting.as_ref())),
        )
        .flatten()
    {
        for expansion in targeting
            .groups
            .iter()
            .filter_map(|group| group.cast_cost_expansion.as_ref())
        {
            if !expansion.condition.expected_selected {
                return Err(
                    "cast-cost target expansion must require its linked option to be selected"
                        .into(),
                );
            }
            validate_cast_cost_condition(&face.cast_cost_groups, &expansion.condition)?;
        }
    }
    validate_effect_list_metadata(&face.spell_effect)?;
    Ok(())
}

impl CardRegistry {
    pub fn from_embedded() -> Result<Self, RegistryError> {
        let mut registry =
            Self::from_chunks_and_tokens(EMBEDDED_RON_CHUNKS, EMBEDDED_TOKEN_CHUNKS)?;
        for &(card_id, card_name, face_id, face_name, oracle_text_sha256) in
            EMBEDDED_PRESENTATION_FACES
        {
            registry.presentation_faces.insert(
                (card_id.to_string(), face_id.to_string()),
                PresentationFaceMetadata {
                    card_name: card_name.to_string(),
                    face_name: face_name.to_string(),
                    oracle_text_sha256: oracle_text_sha256.to_string(),
                },
            );
        }
        Ok(registry)
    }

    #[cfg(test)]
    fn from_chunks(chunks: &[&str]) -> Result<Self, RegistryError> {
        Self::from_chunks_and_tokens(chunks, &[])
    }

    /// Load and validate a complete RON corpus, including its separate token namespace.
    /// Embedded startup and isolated engine fixtures use the same validation path.
    pub fn from_chunks_and_tokens(
        chunks: &[&str],
        token_chunks: &[&str],
    ) -> Result<Self, RegistryError> {
        let mut reg = CardRegistry::default();
        // Tokens first: card effects (CreateTokens) are validated against the token namespace.
        for chunk in token_chunks {
            let token: TokenDefinition = RON_OPTS.from_str(chunk)?;
            let mut def = token.to_card_def();
            def.derive_type_flags();
            let id = token.id.clone();
            if reg.tokens.insert(id.clone(), def).is_some() {
                return Err(RegistryError::InvalidCard {
                    id,
                    reason: "duplicate token id".into(),
                });
            }
        }
        // Token definitions use the same ability vocabulary as permanent cards. Validate after
        // the complete token namespace is loaded so a token trigger may create another token
        // regardless of file ordering.
        for (id, token) in &reg.tokens {
            let face = token.primary_face();
            validate_face_identity(face).map_err(|reason| RegistryError::InvalidCard {
                id: id.clone(),
                reason,
            })?;
            validate_static_abilities(token, face)?;
            validate_saga_face(token, face)?;
            let can_reference_attached_object = face_can_reference_attached_object(face);
            let can_reference_attached_player = face_can_reference_attached_player(face);
            for ability in &face.activated_abilities {
                ability
                    .validate_shape()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: id.clone(),
                        reason,
                    })?;
                for effect in &ability.effect {
                    if let SpellEffectKind::CreateTokens { token, .. }
                    | SpellEffectKind::CreateAttackingTokens { token, .. } = effect
                    {
                        if !reg.tokens.contains_key(token) {
                            return Err(RegistryError::InvalidCard {
                                id: id.clone(),
                                reason: format!("CreateTokens references unknown token '{token}'"),
                            });
                        }
                    }
                }
            }
            for ability in &face.triggered_abilities {
                if ability.trigger.is_delayed_only() {
                    return Err(RegistryError::InvalidCard {
                        id: id.clone(),
                        reason: "delayed trigger conditions require CreateDelayedTrigger".into(),
                    });
                }
                ability
                    .trigger
                    .validate()
                    .and_then(|()| ability.validate_trigger_limit())
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: id.clone(),
                        reason,
                    })?;
                match &ability.trigger {
                    TriggerCondition::WheneverAttachedObjectAttacks
                    | TriggerCondition::WheneverAttachedObjectBecomesTapped
                    | TriggerCondition::WheneverAttachedObjectDies
                        if !can_reference_attached_object =>
                    {
                        return Err(RegistryError::InvalidCard {
                            id: id.clone(),
                            reason: "attached-object trigger requires an object-attaching Aura or Equipment source"
                                .into(),
                        });
                    }
                    TriggerCondition::WheneverAttachedPlayerIsAttacked
                        if !can_reference_attached_player =>
                    {
                        return Err(RegistryError::InvalidCard {
                            id: id.clone(),
                            reason:
                                "attached-player trigger requires a player-attaching Aura source"
                                    .into(),
                        });
                    }
                    _ => {}
                }
                ability
                    .validate_shape()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: id.clone(),
                        reason,
                    })?;
                if ability.effect.is_empty() {
                    return Err(RegistryError::InvalidCard {
                        id: id.clone(),
                        reason: "token triggered ability must contain at least one effect".into(),
                    });
                }
                if let Some(condition) = ability.intervening_if.as_ref() {
                    condition.validate_trigger_condition().map_err(|reason| {
                        RegistryError::InvalidCard {
                            id: id.clone(),
                            reason,
                        }
                    })?;
                }
                for effect in &ability.effect {
                    if effect.uses_trigger_object_reference()
                        && !ability.trigger.supplies_trigger_object()
                    {
                        return Err(RegistryError::InvalidCard {
                            id: id.clone(),
                            reason: "trigger-object effect requires a trigger that supplies an observed object"
                                .into(),
                        });
                    }
                    if effect.uses_defending_player_reference()
                        && !ability.trigger.supplies_defending_player()
                    {
                        return Err(RegistryError::InvalidCard {
                            id: id.clone(),
                            reason: "defending-player target requires an attack trigger that supplies a defender"
                                .into(),
                        });
                    }
                    if effect.uses_attached_object_subject() && !can_reference_attached_object {
                        return Err(RegistryError::InvalidCard {
                            id: id.clone(),
                            reason: "AttachedObject requires an Aura enchanting an object or an Equipment source"
                                .into(),
                        });
                    }
                    effect.validate(EffectContext::Ability).map_err(|reason| {
                        RegistryError::InvalidCard {
                            id: id.clone(),
                            reason,
                        }
                    })?;
                    if let SpellEffectKind::CreateTokens { token, .. }
                    | SpellEffectKind::CreateAttackingTokens { token, .. } = effect
                    {
                        if !reg.tokens.contains_key(token) {
                            return Err(RegistryError::InvalidCard {
                                id: id.clone(),
                                reason: format!("CreateTokens references unknown token '{token}'"),
                            });
                        }
                    }
                }
                SpellEffectKind::validate_list(&ability.effect).map_err(|reason| {
                    RegistryError::InvalidCard {
                        id: id.clone(),
                        reason,
                    }
                })?;
            }
        }
        for chunk in chunks {
            // Authored RON (flat for single-face cards) is normalized into the faces-only runtime
            // shape here — the one place that knows about the flat authoring schema.
            let raw: RawCardDefinition = RON_OPTS.from_str(chunk)?;
            let id = raw.id.clone();
            let mut card = raw
                .into_definition()
                .map_err(|reason| RegistryError::InvalidCard { id, reason })?;
            // Type flags are derived from `types`/`supertypes`, not authored in RON (per face).
            card.derive_type_flags();
            if card.layout == Layout::Adventure {
                let valid_roles = card.faces.len() == 2
                    && card.faces[0].is_permanent()
                    && (card.faces[1].is_instant || card.faces[1].is_sorcery)
                    && !card.faces[1].is_permanent();
                if !valid_roles {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "Adventure requires exactly two faces: permanent face 0 and instant/sorcery face 1"
                            .into(),
                    });
                }
            }
            if card.layout == Layout::Omen {
                let valid_roles = card.faces.len() == 2
                    && card.faces[0].is_permanent()
                    && (card.faces[1].is_instant || card.faces[1].is_sorcery)
                    && !card.faces[1].is_permanent()
                    && card.faces[1].types.iter().any(|value| value == "Omen");
                if !valid_roles {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "Omen requires exactly two faces: permanent face 0 and instant/sorcery Omen face 1"
                            .into(),
                    });
                }
            }
            // Validate every face's effects at startup — multi-face cards (CR 709/712/715/720)
            // validate each face uniformly. Spell effects have no source permanent, so `Source`
            // subjects are rejected here (EffectContext::Spell); activated/triggered
            // effects bind to a source (Ability).
            let mut face_ids = HashSet::new();
            for face in card.faces_iter() {
                validate_face_identity(face).map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
                if !face_ids.insert(face.face_id.as_str()) {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: format!("duplicate face id '{}'", face.face_id),
                    });
                }
                if face.warp_cost.is_some() && (!face.is_permanent() || face.is_land) {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "Warp requires a permanent spell face".into(),
                    });
                }
                for condition in &face.cast_conditions {
                    condition
                        .validate_live()
                        .map_err(|reason| RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        })?;
                }
                for effect in face.spell_effect.iter().chain(
                    face.modal_spell
                        .iter()
                        .flat_map(|modal| &modal.modes)
                        .flat_map(|mode| &mode.effects),
                ) {
                    effect
                        .validate_cast_snapshot_references(face.cast_conditions.len())
                        .map_err(|reason| RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        })?;
                }
                for modifier in &face.cost_modifiers {
                    modifier
                        .validate()
                        .map_err(|reason| RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        })?;
                }
                if let Some(condition) = &face.instant_speed_cast_cost {
                    if !condition.expected_selected {
                        return Err(RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason:
                                "instant-speed cast-cost permission must require a selected option"
                                    .into(),
                        });
                    }
                    validate_cast_cost_condition(&face.cast_cost_groups, condition).map_err(
                        |reason| RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        },
                    )?;
                }
                // One resolution owner per face (CR 608): ordinary data, modal data, and a
                // custom (tier-3) effect are mutually exclusive. The
                // matching custom impl is validated to exist on the `tricerules-core` side
                // (it owns the `CardEffect` lookup; this crate has no engine access).
                let resolution_owners = usize::from(!face.spell_effect.is_empty())
                    + usize::from(face.modal_spell.is_some())
                    + usize::from(face.custom_effect.is_some());
                if resolution_owners > 1 {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "face has more than one of spell_effect, modal_spell, and \
                                 custom_effect (one resolution owner allowed)"
                            .into(),
                    });
                }
                let payment_actions = additional_cost_result_actions(&face.additional_costs);
                for effect in &face.spell_effect {
                    validate_effect_payment_results(&payment_actions, effect).map_err(
                        |reason| RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        },
                    )?;
                    validate_effect_cast_cost_conditions(&face.cast_cost_groups, effect).map_err(
                        |reason| RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        },
                    )?;
                    if effect.uses_defending_player_reference() {
                        return Err(RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason: "spell effects cannot reference a trigger's defending player"
                                .into(),
                        });
                    }
                    effect.validate(EffectContext::Spell).map_err(|reason| {
                        RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        }
                    })?;
                }
                // Rules that depend on sibling effects (e.g. an amount read from another
                // effect's target) can only be checked over the whole list.
                SpellEffectKind::validate_list(&face.spell_effect).map_err(|reason| {
                    RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    }
                })?;
                TargetingDef::validate_optional(face.targeting.as_ref(), &face.spell_effect)
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
                if let Some(modal) = &face.modal_spell {
                    for mode in &modal.modes {
                        for effect in &mode.effects {
                            validate_effect_payment_results(&payment_actions, effect).map_err(
                                |reason| RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason,
                                },
                            )?;
                            validate_effect_cast_cost_conditions(&face.cast_cost_groups, effect)
                                .map_err(|reason| RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason,
                                })?;
                        }
                    }
                    modal.validate(EffectContext::Spell).map_err(|reason| {
                        RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        }
                    })?;
                }
                // CR 604.2: static abilities exist only on permanents (they generate continuous
                // effects while the source is on the battlefield). An instant/sorcery with one is
                // invalid data — its "anthem" belongs in `spell_effect` as a one-shot `PumpAll`.
                if !face.static_abilities.is_empty() && (face.is_instant || face.is_sorcery) {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason:
                            "static_abilities are only valid on permanents (not instant/sorcery)"
                                .into(),
                    });
                }
                let spell_aura_attach_count = face
                    .spell_effect
                    .iter()
                    .filter(|effect| matches!(effect, SpellEffectKind::AuraAttach { .. }))
                    .count();
                let nonspell_aura_attach = face
                    .activated_abilities
                    .iter()
                    .flat_map(|ability| &ability.effect)
                    .chain(
                        face.triggered_abilities
                            .iter()
                            .flat_map(|ability| &ability.effect),
                    )
                    .chain(
                        face.modal_spell
                            .iter()
                            .flat_map(|modal| &modal.modes)
                            .flat_map(|mode| &mode.effects),
                    )
                    .any(|effect| matches!(effect, SpellEffectKind::AuraAttach { .. }));
                if face.is_aura && (spell_aura_attach_count != 1 || nonspell_aura_attach) {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "an Aura face requires exactly one AuraAttach in spell_effect"
                            .into(),
                    });
                }
                if !face.is_aura && (spell_aura_attach_count != 0 || nonspell_aura_attach) {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "AuraAttach is only valid on an Aura face".into(),
                    });
                }
                let uses_attach_source = face
                    .activated_abilities
                    .iter()
                    .flat_map(|ability| &ability.effect)
                    .chain(
                        face.triggered_abilities
                            .iter()
                            .flat_map(|ability| &ability.effect),
                    )
                    .any(|effect| matches!(effect, SpellEffectKind::AttachSource { .. }));
                if uses_attach_source
                    && !face.types.iter().any(|card_type| card_type == "Equipment")
                {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "AttachSource requires an Equipment source".into(),
                    });
                }
                let can_reference_attached_object = face_can_reference_attached_object(face);
                let can_reference_attached_player = face_can_reference_attached_player(face);
                let uses_attached_object = face
                    .activated_abilities
                    .iter()
                    .flat_map(|ability| &ability.effect)
                    .chain(
                        face.triggered_abilities
                            .iter()
                            .flat_map(|ability| &ability.effect),
                    )
                    .any(SpellEffectKind::uses_attached_object_subject);
                if uses_attached_object && !can_reference_attached_object {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "AttachedObject requires an Aura enchanting an object or an Equipment source"
                            .into(),
                    });
                }
                validate_static_abilities(&card, face)?;
                validate_saga_face(&card, face)?;
                for ability in &face.triggered_abilities {
                    if ability.trigger.is_delayed_only() {
                        return Err(RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason: "delayed trigger conditions require CreateDelayedTrigger"
                                .into(),
                        });
                    }
                    ability
                        .trigger
                        .validate()
                        .and_then(|()| ability.validate_trigger_limit())
                        .map_err(|reason| RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        })?;
                    match &ability.trigger {
                        TriggerCondition::WheneverAttachedObjectAttacks
                        | TriggerCondition::WheneverAttachedObjectBecomesTapped
                        | TriggerCondition::WheneverAttachedObjectDies
                        | TriggerCondition::WheneverAttachedObjectDealsCombatDamageToPlayer
                        | TriggerCondition::WheneverAttachedObjectIsDealtDamage
                            if !can_reference_attached_object =>
                        {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: "attached-object trigger requires an object-attaching Aura or Equipment source"
                                    .into(),
                            });
                        }
                        TriggerCondition::WheneverAttachedPlayerIsAttacked
                            if !can_reference_attached_player =>
                        {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: "attached-player trigger requires a player-attaching Aura source"
                                    .into(),
                            });
                        }
                        _ => {}
                    }
                    if ability
                        .effect
                        .iter()
                        .any(SpellEffectKind::uses_trigger_object_reference)
                        && !ability.trigger.supplies_trigger_object()
                    {
                        return Err(RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason: "trigger-object effect requires a trigger that supplies an observed object"
                                .into(),
                        });
                    }
                    if ability
                        .effect
                        .iter()
                        .any(SpellEffectKind::uses_defending_player_reference)
                        && !ability.trigger.supplies_defending_player()
                    {
                        return Err(RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason: "defending-player target requires an attack trigger that supplies a defender"
                                .into(),
                        });
                    }
                    if let Some(condition) = ability.intervening_if.as_ref() {
                        condition.validate_trigger_condition().map_err(|reason| {
                            RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            }
                        })?;
                    }
                }
                // An ability's effect list gets the same two checks a spell's does: each effect
                // against its context, then the list as a whole (CR 608.2 — the effects resolve
                // together, so a cross-effect requirement like `LoseLife(TargetManaValue)` must
                // find its object-targeting sibling inside this one ability).
                for ability in &face.activated_abilities {
                    ability
                        .validate_shape()
                        .map_err(|reason| RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        })?;
                    let allowed = ability_cost_result_actions(&ability.costs);
                    for effect in &ability.effect {
                        validate_effect_payment_results(&allowed, effect).map_err(|reason| {
                            RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            }
                        })?;
                    }
                }
                for ability in &face.triggered_abilities {
                    for effect in &ability.effect {
                        validate_effect_payment_results(&[], effect).map_err(|reason| {
                            RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            }
                        })?;
                    }
                }
                for cost in &face.additional_costs {
                    if matches!(cost, AdditionalCost::Blight { count: 0 }) {
                        return Err(RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason: "additional tap or Blight cost requires a positive count"
                                .into(),
                        });
                    }
                    if let AdditionalCost::TapPermanents {
                        constraint, filter, ..
                    } = cost
                    {
                        constraint
                            .validate_for(ObjectContributionKind::CurrentPower, "additional tap")
                            .map_err(|reason| RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            })?;
                        filter
                            .validate_characteristic_constraints()
                            .map_err(|reason| RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            })?;
                    }
                    if let AdditionalCost::ExileGraveyardCards {
                        constraint, filter, ..
                    } = cost
                    {
                        constraint
                            .validate_for(
                                ObjectContributionKind::ManaValue,
                                "additional graveyard exile",
                            )
                            .and_then(|_| {
                                if constraint.aggregate_minimum().is_some()
                                    && filter == &ZoneCardFilter::default()
                                {
                                    Ok(())
                                } else {
                                    filter.validate()
                                }
                            })
                            .map_err(|reason| RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            })?;
                    }
                    if let AdditionalCost::SacrificePermanent { filter }
                    | AdditionalCost::TapPermanents { filter, .. } = cost
                    {
                        filter
                            .validate_characteristic_constraints()
                            .map_err(|reason| RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            })?;
                        if !filter.all_terminal_filters_match(|leaf| {
                            matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                                && leaf.controller == TargetController::You
                                && leaf.excluded_objects.is_empty()
                        }) {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: "additional selected-permanent cost filter requires Creature or AnyPermanent, controller: You, and may include its source".into(),
                            });
                        }
                    }
                }
                for group in &face.cast_cost_groups {
                    group
                        .validate()
                        .map_err(|reason| RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        })?;
                }
                for effects in face
                    .activated_abilities
                    .iter()
                    .map(|a| &a.effect)
                    .chain(face.triggered_abilities.iter().map(|t| &t.effect))
                {
                    for effect in effects {
                        effect.validate(EffectContext::Ability).map_err(|reason| {
                            RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            }
                        })?;
                    }
                    SpellEffectKind::validate_list(effects).map_err(|reason| {
                        RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        }
                    })?;
                }
                for (effects, targeting) in face
                    .activated_abilities
                    .iter()
                    .map(|ability| (&ability.effect, ability.targeting.as_ref()))
                    .chain(
                        face.triggered_abilities
                            .iter()
                            .map(|ability| (&ability.effect, ability.targeting.as_ref())),
                    )
                {
                    TargetingDef::validate_optional(targeting, effects).map_err(|reason| {
                        RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        }
                    })?;
                }
                // Every CreateTokens effect must name a loaded token (an uncreatable id is a bug).
                let all_effects = face
                    .spell_effect
                    .iter()
                    .chain(face.activated_abilities.iter().flat_map(|a| &a.effect))
                    .chain(face.triggered_abilities.iter().flat_map(|t| &t.effect));
                for effect in all_effects {
                    if let SpellEffectKind::ChangeSourceFace { action } = effect {
                        let valid_layout = match action {
                            FaceChangeAction::Transform => {
                                matches!(card.layout, Layout::Transform | Layout::ModalDfc)
                            }
                            FaceChangeAction::Flip => card.layout == Layout::Flip,
                        };
                        if !valid_layout {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: format!(
                                    "ChangeSourceFace({action:?}) is incompatible with {:?} layout",
                                    card.layout
                                ),
                            });
                        }
                    }
                    if let SpellEffectKind::CreateTokens { token, .. }
                    | SpellEffectKind::CreateAttackingTokens { token, .. } = effect
                    {
                        if !reg.tokens.contains_key(token) {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: format!("CreateTokens references unknown token '{token}'"),
                            });
                        }
                    }
                }
                if let Some(modal) = &face.modal_spell {
                    for effect in modal.modes.iter().flat_map(|mode| &mode.effects) {
                        if let SpellEffectKind::CreateTokens { token, .. }
                        | SpellEffectKind::CreateAttackingTokens { token, .. } = effect
                        {
                            if !reg.tokens.contains_key(token) {
                                return Err(RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason: format!(
                                        "CreateTokens references unknown token '{token}'"
                                    ),
                                });
                            }
                        }
                    }
                }
            }
            let id = card.id.clone();
            // Index the whole-card name (what decks/`cards.xml` reference) and, for multi-face
            // cards, each face name, so `id_for_name` resolves either half to the one card id.
            let mut names: Vec<String> = vec![card.name.clone()];
            if card.is_multiface() {
                names.extend(card.faces.iter().map(|f| f.name.clone()));
            }
            for name in names {
                if reg
                    .by_name
                    .insert(normalize_name(&name), id.clone())
                    .is_some()
                {
                    return Err(RegistryError::InvalidCard {
                        id,
                        reason: format!("duplicate name '{name}'"),
                    });
                }
            }
            if reg.by_id.insert(id.clone(), card).is_some() {
                return Err(RegistryError::InvalidCard {
                    id,
                    reason: "duplicate id".into(),
                });
            }
        }
        Ok(reg)
    }

    /// Look up a definition by id. Falls back to the token namespace so the engine queries a
    /// token object's characteristics (types, P/T, keywords, colors) the same way as a card.
    pub fn get(&self, id: &str) -> Option<&CardDefinition> {
        self.by_id.get(id).or_else(|| self.tokens.get(id))
    }

    /// True if `id` names a token (created by an effect), not a deck card.
    pub fn is_token(&self, id: &str) -> bool {
        self.tokens.contains_key(id)
    }

    /// Resolves an Oracle card name (trimmed, case-insensitive) to a card id.
    /// This is the only supported name->id path; deck lists cross IPC as names.
    pub fn id_for_name(&self, name: &str) -> Option<&str> {
        self.by_name.get(&normalize_name(name)).map(String::as_str)
    }

    /// Iterate over every loaded card definition (order is unspecified).
    pub fn definitions(&self) -> impl Iterator<Item = &CardDefinition> {
        self.by_id.values()
    }

    pub fn presentation_face(
        &self,
        card_id: &str,
        face_id: &str,
    ) -> Option<&PresentationFaceMetadata> {
        self.presentation_faces
            .get(&(card_id.to_string(), face_id.to_string()))
    }

    pub fn global() -> &'static CardRegistry {
        &GLOBAL
    }

    /// Stable FNV-1a hash of the sorted embedded card RON, as 16-char hex.
    /// Build-to-build stable (not cryptographic): a card-data version tag for the
    /// Servatrice↔sidecar handshake and ruled-replay stamping, so (seed, command log,
    /// data hash) reproduces a game. `EMBEDDED_RON_CHUNKS` is path-sorted by build.rs,
    /// so the hash is independent of filesystem enumeration order.
    pub fn content_hash() -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a offset basis
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        // Tokens are part of the rules data, so fold them into the hash after cards.
        for chunk in EMBEDDED_RON_CHUNKS
            .iter()
            .chain(EMBEDDED_TOKEN_CHUNKS.iter())
        {
            for &b in chunk.as_bytes() {
                hash ^= b as u64;
                hash = hash.wrapping_mul(PRIME);
            }
            hash ^= 0xff; // chunk separator so file boundaries are significant
            hash = hash.wrapping_mul(PRIME);
        }
        format!("{hash:016x}")
    }
}

// `EMBEDDED_RON_CHUNKS`, generated by build.rs from `data/**/*.ron`.
include!(concat!(env!("OUT_DIR"), "/embedded_cards.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::{
        Amount, BattlefieldCreatureCountFilter, CastTriggerPlayer, CombatRole, CountExpression,
        CounterKind, CreatureEventFilter, CreatureScopeController, CreatureScopeFilter,
        EffectSubject, EntersTappedAffected, EntersWithCountersAffected, GameCondition, ManaAmount,
        PermanentTypeFilter, PlayerLifeAggregate, PlayerRecipient, PowerComparison,
        RelativePlayerSet, SpellCostModifier, SpellEffectKind, StaticAbilityDef, TargetFilter,
        TargetKind, TriggerCondition,
    };

    #[test]
    fn embedded_registry_loads() {
        CardRegistry::from_embedded().unwrap();
    }

    #[test]
    fn issue_171_keyword_choices_are_validated() {
        for (choices, valid) in [
            ("[Menace, Lifelink]", true),
            ("[]", false),
            ("[Menace]", false),
            ("[Menace, Menace]", false),
        ] {
            let card = format!(
                r#"(id: "choice_test", name: "Choice Test", face_id: "choice_test", types: ["Instant"],
                spell_effect: [GrantKeywordChoice(subject: Chosen((kind: Creature)), choices: {choices})])"#
            );
            assert_eq!(
                CardRegistry::from_chunks(&[&card]).is_ok(),
                valid,
                "{choices}"
            );
        }
    }

    #[test]
    fn issue_171_crime_count_bounds_are_validated() {
        for (bounds, valid) in [
            ("min: Some(1)", true),
            ("max: Some(0)", true),
            ("min: Some(2), max: Some(1)", false),
            ("", false),
        ] {
            let card = format!(
                r#"(id: "crime_test", name: "Crime Test", face_id: "crime_test", types: ["Creature"], power: 1, toughness: 1,
                triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback, trigger: WhenSelfEntersBattlefield, effect: [GainLife(amount: 1)],
                    intervening_if: Some(CrimesCommittedThisTurn(players: Controller, {bounds})) )])"#
            );
            assert_eq!(
                CardRegistry::from_chunks(&[&card]).is_ok(),
                valid,
                "{bounds}"
            );
        }
    }

    #[test]
    fn issue_167_history_bounds_and_permanent_types_are_validated() {
        for kind in [
            "PermanentCardsEnteredGraveyardThisTurn",
            "PermanentsSacrificedThisTurn",
        ] {
            for (fields, valid) in [
                ("min: Some(1)", true),
                ("max: Some(0), permanent_type: Some(Creature)", true),
                (
                    "min: Some(1), max: Some(1), permanent_type: Some(Artifact)",
                    true,
                ),
                ("min: Some(2), max: Some(1)", false),
                ("min: Some(1), permanent_type: Some(Instant)", false),
                ("", false),
            ] {
                let data = format!(
                    r#"(id: "history_probe", name: "History Probe", face_id: "history_probe", types: ["Instant"],
                    cast_conditions: [{kind}(players: Controller, {fields})], spell_effect: [GainLife(amount: 1)])"#
                );
                assert_eq!(
                    CardRegistry::from_chunks(&[&data]).is_ok(),
                    valid,
                    "{kind}: {fields}"
                );
            }
        }
    }

    #[test]
    fn issue_164_rejects_zero_trigger_caps_in_printed_and_granted_abilities() {
        let ability = r#"(ability_id: "triggered_01", presentation: Fallback, trigger: WhenSelfEntersBattlefield, effect: [GainLife(amount: 1)], max_triggers_per_turn: Some(0))"#;
        for fields in [
            format!("triggered_abilities: [{ability}]"),
            format!("static_abilities: [(ability_id: \"static_01\", presentation: Fallback, definition: ConditionalSelfModifier(condition: ActivePlayer(players: Controller), triggered_abilities: [{ability}]))]"),
            format!("activated_abilities: [(ability_id: \"activated_01\", presentation: Fallback, costs: [], effect: [GrantTriggeredAbility(subject: Source, ability: {ability})])]"),
        ] {
            let card = format!(r#"(id: "trigger_limit_test", name: "Trigger Limit Test", face_id: "trigger_limit_test", mana_cost: "{{1}}", types: ["Enchantment"], {fields})"#);
            let error = CardRegistry::from_chunks(&[&card]).expect_err("zero cap must be rejected");
            assert!(matches!(&error,
                RegistryError::InvalidCard { reason, .. } if reason.contains("max_triggers_per_turn")),
                "a zero trigger cap must fail shape validation: {fields}: {error}");
        }
        let token = format!(
            r#"(id: "trigger_limit_test", name: "Trigger Limit Test", face_id: "trigger_limit_test", types: ["Creature"], power: Some(1), toughness: Some(1), triggered_abilities: [{ability}])"#
        );
        let error = CardRegistry::from_chunks_and_tokens(&[], &[&token])
            .expect_err("zero cap on a token must be rejected");
        assert!(
            matches!(error, RegistryError::InvalidCard { reason, .. } if reason.contains("max_triggers_per_turn"))
        );
    }

    #[test]
    fn issue_164_trigger_caps_preserve_defaults_and_lifetime_limits() {
        for (limit, expected) in [
            ("", None),
            ("max_triggers_per_turn: Some(1),", Some(1)),
            ("max_triggers_per_turn: Some(2),", Some(2)),
        ] {
            for lifetime in [false, true] {
                let card = format!(
                    r#"(id: "trigger_limit_test", name: "Trigger Limit Test", face_id: "trigger_limit_test", mana_cost: "{{1}}", types: ["Enchantment"], triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback, trigger: WhenSelfEntersBattlefield, effect: [GainLife(amount: 1)], triggers_only_once: {lifetime}, {limit})])"#
                );
                let registry = CardRegistry::from_chunks(&[&card]).unwrap();
                let ability = &registry
                    .get("trigger_limit_test")
                    .unwrap()
                    .primary_face()
                    .triggered_abilities[0];
                assert_eq!(ability.max_triggers_per_turn, expected);
                assert_eq!(ability.triggers_only_once, lifetime);
            }
        }
    }

    #[test]
    fn issue_160_cards_use_typed_combat_constraints() {
        let registry = CardRegistry::from_embedded().expect("embedded registry");

        let dark_endurance = registry
            .get("dark_endurance")
            .expect("Dark Endurance")
            .primary_face();
        assert!(matches!(
            dark_endurance.cost_modifiers.as_slice(),
            [SpellCostModifier::TargetMatchGenericReduction {
                amount: 1,
                filter: crate::TargetMatchFilter::Battlefield(TargetFilter {
                    combat_role: Some(CombatRole::Blocking),
                    ..
                }),
            }]
        ));

        let vinebender = registry
            .get("foggy_swamp_vinebender")
            .expect("Foggy Swamp Vinebender");
        let [vinebender_restriction] = vinebender.primary_face().static_abilities.as_slice() else {
            panic!("expected one static ability");
        };
        assert!(matches!(
            &vinebender_restriction.definition,
            StaticAbilityDef::SelfCombatRestriction { restriction, .. }
                if restriction.cant_be_blocked_by[0].power == Some(PowerComparison::AtMost(2))
        ));

        let cavalry = registry
            .get("safewright_cavalry")
            .expect("Safewright Cavalry");
        let [cavalry_restriction] = cavalry.primary_face().static_abilities.as_slice() else {
            panic!("expected one static ability");
        };
        assert!(matches!(
            &cavalry_restriction.definition,
            StaticAbilityDef::SelfCombatRestriction { restriction, .. } if restriction.maximum_blockers == Some(1)
        ));
    }

    #[test]
    fn issue_161_shared_turn_boundary_vocabulary_deserializes() {
        let card = r#"(
            id: "issue_161_turn_boundary_card",
            name: "Issue 161 Turn Boundary Card",
            face_id: "issue_161_turn_boundary_card",
            mana_cost: "{3}",
            types: ["Artifact"],
            static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: UntapsDuringOtherPlayersUntapSteps)],
            triggered_abilities: [(
                ability_id: "triggered_01",
                presentation: Fallback,
                trigger: WhenSelfEntersBattlefield,
                effect: [CreateTokens(
                    token: "issue_161_token",
                    count: 1,
                    sacrifice_timing: Some(ControllerNextTurnEndStep),
                )],
            )],
        )"#;
        let token = r#"(
            id: "issue_161_token",
            name: "Issue 161 Token",
            face_id: "issue_161_token",
            types: ["Artifact"],
        )"#;

        CardRegistry::from_chunks_and_tokens(&[card], &[token])
            .expect("issue 161 shared vocabulary must load");
    }

    #[test]
    fn issue_162_tapped_ordinary_token_vocabulary_deserializes() {
        let card = r#"(
            id: "issue_162_tapped_token_maker",
            name: "Issue 162 Tapped Token Maker",
            face_id: "issue_162_tapped_token_maker",
            mana_cost: "{2}",
            types: ["Sorcery"],
            spell_effect: [CreateTokens(
                token: "issue_162_robot",
                count: 2,
                tapped: true,
            )],
        )"#;
        let token = r#"(
            id: "issue_162_robot",
            name: "Issue 162 Robot",
            face_id: "issue_162_robot",
            types: ["Artifact", "Creature", "Robot"],
            power: 2,
            toughness: 2,
        )"#;

        let registry = CardRegistry::from_chunks_and_tokens(&[card], &[token])
            .expect("ordinary token creation must accept an authored tapped flag");
        let effect = &registry
            .get("issue_162_tapped_token_maker")
            .expect("test card")
            .primary_face()
            .spell_effect[0];
        assert!(matches!(
            effect,
            SpellEffectKind::CreateTokens { tapped: true, .. }
        ));
    }

    #[test]
    fn previous_card_result_rejects_an_incompatible_preceding_effect() {
        let card = r#"(
            id: "bad_previous_result",
            name: "Bad Previous Result",
            face_id: "bad_previous_result",
            mana_cost: "{1}",
            types: ["Sorcery"],
            spell_effect: [
                Draw(count: 1),
                GainLife(amount: Count(CardsMatchingResult(filter: (
                    source: PreviousEffect,
                    action: Discard,
                    players: All,
                    card_type: None,
                )))),
            ],
        )"#;

        let error = CardRegistry::from_chunks(&[card]).expect_err("invalid result dependency");
        assert!(error
            .to_string()
            .contains("immediately preceding compatible"));
    }

    #[test]
    fn payment_card_result_requires_a_compatible_authored_cost() {
        let card = r#"(
            id: "bad_payment_result",
            name: "Bad Payment Result",
            face_id: "bad_payment_result",
            mana_cost: "{1}",
            types: ["Sorcery"],
            spell_effect: [ChooseResolutionBranch(
                selection: FirstApplicable,
                branches: [
                    (
                        branch_id: "paid_discard",
                        presentation: Fallback,
                        cost: None,
                        requirement: CardResultCount(
                            filter: (
                                source: Payment,
                                action: Discard,
                                players: Controller,
                                card_type: None,
                            ),
                            min: Some(1),
                        ),
                        effects: [],
                    ),
                    (branch_id: "fallback_branch", presentation: Fallback, cost: None, requirement: Always, effects: []),
                ],
            )],
        )"#;

        let error = CardRegistry::from_chunks(&[card]).expect_err("missing discard cost");
        assert!(error.to_string().contains("compatible card cost"));
    }

    #[test]
    fn issue_125_damage_spells_share_their_target_with_the_death_replacement() {
        let registry = CardRegistry::from_embedded().expect("embedded registry");
        for (id, expected_damage) in [("lava_coil", 4), ("scorching_dragonfire", 3)] {
            let card = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
            let face = card.primary_face();
            let [SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(amount),
                target: damage_target,
            }, SpellEffectKind::ExileIfWouldDieThisTurn {
                target: replacement_target,
            }] = face.spell_effect.as_slice()
            else {
                panic!("{id} must damage and establish the matching death replacement")
            };
            assert_eq!(*amount, expected_damage);
            assert_eq!(damage_target, replacement_target);
            if id == "lava_coil" {
                assert_eq!(damage_target.kind, TargetKind::Creature);
            } else {
                assert!(matches!(
                    damage_target.any_of.as_deref(),
                    Some([
                        TargetFilter {
                            kind: TargetKind::Creature,
                            ..
                        },
                        TargetFilter {
                            kind: TargetKind::AnyPermanent,
                            permanent_types,
                            ..
                        },
                    ]) if *permanent_types == [PermanentTypeFilter::Planeswalker]
                ));
            }
            let targeting = face.targeting.as_ref().expect("explicit grouped target");
            assert_eq!(targeting.groups.len(), 1);
            assert_eq!(targeting.groups[0].min, 1);
            assert_eq!(targeting.groups[0].max, 1);
            assert_eq!(targeting.groups[0].effect_indices, [0, 1]);
        }
    }

    #[test]
    fn winged_words_loads_its_conditional_reduction() {
        let registry = CardRegistry::from_embedded().expect("embedded registry");
        let card = registry.get("winged_words").expect("Winged Words");
        assert!(matches!(
            card.primary_face().cost_modifiers.as_slice(),
            [SpellCostModifier::ConditionalGenericReduction {
                amount: 1,
                condition: GameCondition::BattlefieldCreatureCount {
                    min: Some(1),
                    max: None,
                    ..
                },
            }]
        ));
    }

    #[test]
    fn issue_148_warp_cost_survives_flat_face_normalization() {
        let fixture = r#"(id: "warp_test", name: "Warp Test", face_id: "warp_test", mana_cost: "{3}{W}",
            warp_cost: Some("{1}{W}"), types: ["Creature"], power: 3, toughness: 2)"#;
        let registry = CardRegistry::from_chunks(&[fixture]).expect("Warp face");
        let face = registry.get("warp_test").unwrap().primary_face();
        let serialized = ron::to_string(face).unwrap();
        assert!(
            serialized.contains("warp_cost:Some(\"{1}{W}\")"),
            "Warp must be retained on the normalized face: {serialized}"
        );
    }

    #[test]
    fn issue_148_warp_rejects_nonpermanent_faces() {
        let fixture = r#"(id: "bad_warp", name: "Bad Warp", face_id: "bad_warp", mana_cost: "{3}{W}",
            warp_cost: Some("{1}{W}"), types: ["Sorcery"], spell_effect: [Draw(count: 1)])"#;
        assert!(matches!(CardRegistry::from_chunks(&[fixture]),
            Err(RegistryError::InvalidCard { reason, .. }) if reason.contains("Warp")));
    }

    #[test]
    fn conditional_reductions_allow_x_and_target_count_increases() {
        let x_cost = r#"(
            id: "bad_x_reduction",
            name: "Bad X Reduction",
            face_id: "bad_x_reduction",
            mana_cost: "{X}{U}",
            cost_modifiers: [ConditionalGenericReduction(
                amount: 1,
                condition: BattlefieldCreatureCount(
                    filter: (controllers: Controller, required_keywords: [Flying]),
                    min: Some(1),
                ),
            )],
            types: ["Sorcery"],
            spell_effect: [Draw(count: 1)],
        )"#;
        CardRegistry::from_chunks(&[x_cost]).expect("X is quoted separately from reductions");

        let target_surcharge = r#"(
            id: "bad_target_surcharge_reduction",
            name: "Bad Target Surcharge Reduction",
            face_id: "bad_target_surcharge_reduction",
            mana_cost: "{U}",
            cost_modifiers: [ConditionalGenericReduction(
                amount: 1,
                condition: BattlefieldCreatureCount(
                    filter: (controllers: Controller, required_keywords: [Flying]),
                    min: Some(1),
                ),
            )],
            types: ["Sorcery"],
            spell_effect: [DamageTargets(
                amount: 1,
                target: (kind: AnyTarget),
                division: EvenAtResolution,
                extra_mana_per_target: 1,
            )],
        )"#;
        CardRegistry::from_chunks(&[target_surcharge])
            .expect("target-count increases are quoted before reductions");
    }

    #[test]
    fn block_trigger_filters_reject_contradictory_keywords() {
        let card = r#"(
            id: "bad_block_filter",
            name: "Bad Block Filter",
            face_id: "bad_block_filter",
            mana_cost: "{G}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                ability_id: "triggered_01",
                presentation: Fallback,
                trigger: WheneverSelfBlocksCreature(attacker: (
                    required_keywords: [Flying],
                    excluded_keywords: [Flying],
                )),
                effect: [PumpTarget(power: 1, toughness: 0, subject: Source)],
            )],
        )"#;
        let error = CardRegistry::from_chunks(&[card]).expect_err("contradictory filter");
        assert!(matches!(
            error,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("cannot require and exclude the same keyword")
        ));
    }

    #[test]
    fn issue_127_embedded_cards_load_event_time_power_filters() {
        let registry = CardRegistry::from_embedded().expect("embedded registry");

        for card_id in ["vicious_clown", "mentor_of_the_meek"] {
            let card = registry
                .get(card_id)
                .unwrap_or_else(|| panic!("missing {card_id}"));
            assert!(matches!(
                card.primary_face().triggered_abilities.as_slice(),
                [ability]
                    if ability.trigger
                        == TriggerCondition::WheneverPermanentEntersBattlefield {
                            controller: CastTriggerPlayer::Controller,


                            creature_filter: Some(CreatureEventFilter {
                                power: Some(PowerComparison::AtMost(2)),
                                ..Default::default()
                            }), filter: crate::primitives::PermanentEventFilter { permanent_type: Some(PermanentTypeFilter::Creature), exclude_source: true, ..Default::default() },}
            ));
        }
    }

    #[test]
    fn issue_127_entry_creature_filters_require_a_creature_type_and_a_predicate() {
        for (permanent_type, creature_filter, expected_reason) in [
            (
                "Land",
                "(power: Some(AtMost(2)))",
                "requires permanent_type Creature",
            ),
            ("Creature", "()", "cannot be empty"),
        ] {
            let card = format!(
                r#"(
                    id: "bad_entry_filter",
                    name: "Bad Entry Filter",
                    face_id: "bad_entry_filter",
                    mana_cost: "{{1}}{{G}}",
                    types: ["Creature"],
                    power: 1,
                    toughness: 1,
                    triggered_abilities: [(
                        ability_id: "triggered_01",
                        presentation: Fallback,
                        trigger: WheneverPermanentEntersBattlefield(
                            filter: (permanent_type: Some({permanent_type})),
                            creature_filter: Some({creature_filter}),
                        ),
                        effect: [Draw(count: 1)],
                    )],
                )"#
            );
            let error = CardRegistry::from_chunks(&[&card]).expect_err("invalid entry filter");
            assert!(matches!(
                error,
                RegistryError::InvalidCard { reason, .. } if reason.contains(expected_reason)
            ));
        }
    }

    #[test]
    fn trigger_object_references_require_an_object_supplying_trigger() {
        let card = r#"(
            id: "bad_trigger_reference",
            name: "Bad Trigger Reference",
            face_id: "bad_trigger_reference",
            mana_cost: "{G}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                ability_id: "triggered_01",
                presentation: Fallback,
                trigger: WhenSelfEntersBattlefield,
                effect: [LoseLife(amount: Fixed(2), who: TriggerObjectController)],
            )],
        )"#;
        let error = CardRegistry::from_chunks(&[card]).expect_err("missing trigger object");
        assert!(matches!(
            error,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("requires a trigger that supplies an observed object")
        ));
    }

    #[test]
    fn attachment_trigger_shapes_and_event_context_are_validated() {
        let ordinary_permanent = r#"(
            id: "bad_attachment_trigger",
            name: "Bad Attachment Trigger",
            face_id: "bad_attachment_trigger",
            mana_cost: "{2}",
            types: ["Artifact"],
            triggered_abilities: [(
                ability_id: "triggered_01",
                presentation: Fallback,
                trigger: WheneverAttachedObjectAttacks,
                effect: [Draw(count: 1)],
            )],
        )"#;
        let error = CardRegistry::from_chunks(&[ordinary_permanent])
            .expect_err("ordinary permanent cannot carry an attachment trigger");
        assert!(matches!(
            error,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("object-attaching Aura or Equipment")
        ));

        let wrong_aura_recipient = r#"(
            id: "bad_player_attachment_trigger",
            name: "Bad Player Attachment Trigger",
            face_id: "bad_player_attachment_trigger",
            mana_cost: "{1}{U}",
            types: ["Enchantment", "Aura"],
            spell_effect: [AuraAttach(target: (kind: Creature))],
            triggered_abilities: [(
                ability_id: "triggered_01",
                presentation: Fallback,
                trigger: WheneverAttachedPlayerIsAttacked,
                effect: [Draw(count: 1)],
            )],
        )"#;
        let error = CardRegistry::from_chunks(&[wrong_aura_recipient])
            .expect_err("creature Aura cannot observe an attached player");
        assert!(matches!(
            error,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("player-attaching Aura")
        ));

        let missing_defender = r#"(
            id: "bad_defender_target",
            name: "Bad Defender Target",
            face_id: "bad_defender_target",
            mana_cost: "{1}{R}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                ability_id: "triggered_01",
                presentation: Fallback,
                trigger: WhenSelfEntersBattlefield,
                effect: [DamageTarget(
                    amount: 1,
                    target: (kind: Creature, controller: DefendingPlayer),
                )],
            )],
        )"#;
        let error = CardRegistry::from_chunks(&[missing_defender])
            .expect_err("nonattack trigger has no defending player");
        assert!(matches!(
            error,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("attack trigger that supplies a defender")
        ));
    }

    #[test]
    fn triggered_graveyard_return_rejects_invalid_entry_counter_lists() {
        for entry_counters in [
            "[(counter: PlusOnePlusOne, count: 0)]",
            "[(counter: PlusOnePlusOne, count: 1), (counter: PlusOnePlusOne, count: 2)]",
        ] {
            let card = format!(
                r#"(
                    id: "bad_entry_counters",
                    name: "Bad Entry Counters",
                    face_id: "bad_entry_counters",
                    mana_cost: "{{1}}{{B}}",
                    types: ["Enchantment", "Aura"],
                    spell_effect: [AuraAttach(target: (kind: Creature))],
                    triggered_abilities: [(
                        ability_id: "triggered_01",
                        presentation: Fallback,
                        trigger: WheneverAttachedObjectDies,
                        effect: [ReturnTriggeredCard(
                            from: [Graveyard],
                            reference: TriggerObject,
                            controller: AbilityController,
                            entry_counters: {entry_counters},
                        )],
                    )],
                )"#
            );
            let error = CardRegistry::from_chunks(&[&card])
                .expect_err("invalid entry counter list must fail registry load");
            assert!(matches!(
                error,
                RegistryError::InvalidCard { reason, .. }
                    if reason.contains("entry counter")
            ));
        }
    }

    #[test]
    fn counter_kinds_accept_supported_keyword_and_stun_but_reject_illegal_keyword_counters() {
        for counter in ["Keyword(Flying)", "Stun"] {
            let card = format!(
                r#"(
                    id: "valid_counter_kind",
                    name: "Valid Counter Kind",
                    face_id: "valid_counter_kind",
                    mana_cost: "{{G}}",
                    types: ["Instant"],
                    spell_effect: [PutCounters(counter: {counter}, count: 1)],
                )"#
            );
            CardRegistry::from_chunks(&[&card]).expect("supported counter kind must load");
        }

        let invalid = r#"(
            id: "invalid_keyword_counter",
            name: "Invalid Keyword Counter",
            face_id: "invalid_keyword_counter",
            mana_cost: "{G}",
            types: ["Instant"],
            spell_effect: [PutCounters(counter: Keyword(Defender), count: 1)],
        )"#;
        let error = CardRegistry::from_chunks(&[invalid])
            .expect_err("unsupported keyword counter must fail registry load");
        assert!(
            matches!(
                &error,
                RegistryError::InvalidCard { reason, .. }
                    if reason.contains("not a legal keyword-counter kind")
            ),
            "unexpected registry error: {error:?}"
        );
    }

    #[test]
    fn conditional_self_modifier_rejects_empty_and_recursive_characteristic_shapes() {
        let empty = r#"(
            id: "bad_conditional",
            name: "Bad Conditional",
            face_id: "bad_conditional",
            mana_cost: "{G}",
            types: ["Creature", "Test"],
            power: 1,
            toughness: 1,
            static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: ConditionalSelfModifier(
                condition: ActivePlayer(players: Controller),
            ))],
        )"#;
        let err = CardRegistry::from_chunks_and_tokens(&[empty], &[]).unwrap_err();
        assert!(matches!(
            err,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("must modify at least one value")
        ));

        let recursive = r#"(
            id: "bad_recursive_conditional",
            name: "Bad Recursive Conditional",
            face_id: "bad_recursive_conditional",
            mana_cost: "{G}",
            types: ["Creature", "Test"],
            power: 1,
            toughness: 1,
            static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: ConditionalSelfModifier(
                condition: BattlefieldAggregate(
                    filter: (controllers: Controller, card_type: Some(Creature)),
                    aggregate: MaximumPower,
                    min: Some(4),
                ),
                delta_power: 1,
            ))],
        )"#;
        let err = CardRegistry::from_chunks_and_tokens(&[recursive], &[]).unwrap_err();
        assert!(matches!(
            err,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("CR 613.8 dependency ordering")
        ));

        let nested_recursive = r#"(
            id: "bad_nested_recursive_conditional",
            name: "Bad Nested Recursive Conditional",
            face_id: "bad_nested_recursive_conditional",
            mana_cost: "{G}",
            types: ["Creature", "Test"],
            power: 1,
            toughness: 1,
            static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: ConditionalSelfModifier(
                condition: AnyOf([
                    ActivePlayer(players: Controller),
                    BattlefieldAggregate(
                        filter: (controllers: Controller, card_type: Some(Creature)),
                        aggregate: MaximumPower,
                        min: Some(4),
                    ),
                ]),
                delta_power: 1,
            ))],
        )"#;
        let err = CardRegistry::from_chunks_and_tokens(&[nested_recursive], &[]).unwrap_err();
        assert!(matches!(
            err,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("CR 613.8 dependency ordering")
        ));
    }

    #[test]
    fn issue_115_rejects_invalid_attachment_observer_and_condition_shapes() {
        let bad_trigger = r#"(
            id: "bad_damage_observer",
            name: "Bad Damage Observer",
            face_id: "bad_damage_observer",
            mana_cost: "{2}",
            types: ["Artifact"],
            triggered_abilities: [(
                ability_id: "triggered_01",
                presentation: Fallback,
                trigger: WheneverAttachedObjectIsDealtDamage,
                effect: [Draw(count: 1)],
            )],
        )"#;
        let error = CardRegistry::from_chunks(&[bad_trigger]).expect_err("nonattachment observer");
        assert!(matches!(
            error,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("object-attaching Aura or Equipment")
        ));

        let conditioned_ability = r#"(
            id: "bad_conditioned_grant",
            name: "Bad Conditioned Grant",
            face_id: "bad_conditioned_grant",
            mana_cost: "{2}",
            types: ["Artifact", "Equipment"],
            static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: AttachedModifier(
                condition: Some(ActivePlayer(players: Controller)),
                activated_abilities: [(
                    ability_id: "triggered_01",
                    presentation: Fallback,
                    costs: [Tap],
                    effect: [ProduceMana(options: [(g: 1)])],
                )],
            ))],
        )"#;
        let error = CardRegistry::from_chunks(&[conditioned_ability])
            .expect_err("conditioned ability grant");
        assert!(matches!(
            error,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("only supports characteristic modifiers")
        ));

        let power_dependency = r#"(
            id: "bad_attached_dependency",
            name: "Bad Attached Dependency",
            face_id: "bad_attached_dependency",
            mana_cost: "{2}",
            types: ["Artifact", "Equipment"],
            static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: AttachedModifier(
                condition: Some(BattlefieldAggregate(
                    filter: (controllers: Controller, card_type: Some(Creature)),
                    aggregate: MaximumPower,
                    min: Some(4),
                )),
                keywords: [FirstStrike],
            ))],
        )"#;
        let error = CardRegistry::from_chunks(&[power_dependency])
            .expect_err("power-dependent attached characteristics");
        assert!(matches!(
            error,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("CR 613.8 dependency ordering")
        ));
    }

    #[test]
    fn issue_116_rejects_power_dependent_conditioned_anthems() {
        let power_dependency = r#"(
            id: "bad_conditioned_anthem",
            name: "Bad Conditioned Anthem",
            face_id: "bad_conditioned_anthem",
            mana_cost: "{2}{W}",
            types: ["Creature", "Test"],
            power: 2,
            toughness: 2,
            static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: AnthemKeyword(
                filter: (controller: YouControl),
                condition: BattlefieldAggregate(
                    filter: (controllers: Controller, card_type: Some(Creature)),
                    aggregate: MaximumPower,
                    min: Some(4),
                ),
                keyword: FirstStrike,
            ))],
        )"#;
        let error = CardRegistry::from_chunks(&[power_dependency])
            .expect_err("power-dependent conditioned anthem");
        assert!(matches!(
            error,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("CR 613.8 dependency ordering")
        ));
    }

    #[test]
    fn token_trigger_validation_rejects_empty_effects() {
        let empty_effect = r#"(
            id: "bad_token",
            name: "Bad",
            face_id: "bad",
            types: ["Creature", "Bad"],
            colors: [Red],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                ability_id: "triggered_01",
                presentation: Fallback,
                trigger: WheneverPlayerCastsSpell(caster: Controller, filter: (card_type: Some(Noncreature))),
                effect: [],
            )],
        )"#;
        let err = CardRegistry::from_chunks_and_tokens(&[], &[empty_effect]).unwrap_err();
        assert!(matches!(
            err,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("requires exactly one of effect or modal")
        ));
    }

    #[test]
    fn target_filter_rejects_required_and_excluded_keyword_overlap() {
        let bad = r#"(
            id: "contradictory_filter",
            name: "Contradictory Filter",
            face_id: "contradictory_filter",
            mana_cost: "{W}",
            types: ["Instant"],
            spell_effect: [Destroy(subject: Chosen((
                kind: Creature,
                required_keywords: [Flying],
                excluded_keywords: [Flying],
            )))],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).expect_err("overlap must be rejected");
        assert!(matches!(
            err,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("both require and exclude keyword Flying")
        ));
    }

    #[test]
    fn spell_effects_deserialize_from_ron() {
        let reg = CardRegistry::from_embedded().unwrap();
        assert_eq!(
            reg.get("angels_mercy").unwrap().primary_face().spell_effect,
            vec![SpellEffectKind::GainLife {
                amount: Amount::Fixed(7)
            }]
        );
        assert_eq!(
            reg.get("lightning_bolt")
                .unwrap()
                .primary_face()
                .spell_effect,
            vec![SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(3),
                target: TargetFilter {
                    kind: TargetKind::AnyTarget,
                    ..Default::default()
                },
            }]
        );
        assert_eq!(
            reg.get("mind_sculpt").unwrap().primary_face().spell_effect,
            vec![SpellEffectKind::MillTargetPlayer {
                count: 7,
                target: TargetFilter {
                    kind: TargetKind::OpponentPlayer,
                    ..Default::default()
                },
            }]
        );
        assert_eq!(
            reg.get("crippling_chill")
                .unwrap()
                .primary_face()
                .spell_effect,
            vec![
                SpellEffectKind::Tap {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                },
                SpellEffectKind::SkipNextUntap {
                    target: TargetFilter::default_creature(),
                },
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                },
            ]
        );
    }

    #[test]
    fn startup_validation_rejects_incompatible_target_filter() {
        // A player-life effect pointed at a creature is invalid card data.
        let bad = r#"(
            id: "bad_card",
            name: "Bad Card",
            face_id: "bad_card",
            mana_cost: "{W}",
            types: ["Instant"],
            spell_effect: [TargetPlayerGainsLife(amount: 3, target: (kind: Creature))],
        )"#;
        let raw: RawCardDefinition = RON_OPTS.from_str(bad).unwrap();
        let card = raw.into_definition().unwrap();
        assert!(card.primary_face().spell_effect[0]
            .validate(crate::primitives::EffectContext::Spell)
            .is_err());
    }

    #[test]
    fn load_rejects_creature_scoped_combat_prevention_aimed_at_a_player() {
        let bad = r#"(
            id: "bad_combat_prevention",
            name: "Bad Combat Prevention",
            face_id: "bad_combat_prevention",
            mana_cost: "{W}",
            types: ["Instant"],
            spell_effect: [
                PreventAllCombatDamageToTargetTurn(target: (kind: AnyPlayer)),
            ],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(matches!(
            err,
            RegistryError::InvalidCard { ref reason, .. }
                if reason.contains("requires a creature target filter")
        ));
    }

    /// CR 701.19 / 701.20: tapping and untapping act on permanents, so a player-kind filter on
    /// either is invalid card data and must not survive registry load.
    #[test]
    fn load_rejects_tap_or_untap_aimed_at_a_player() {
        for effect in ["Tap", "Untap", "SkipNextUntap"] {
            let bad = format!(
                r#"(
            id: "bad_{}",
            name: "Bad {}",
            face_id: "bad",
            mana_cost: "{{U}}",
            types: ["Instant"],
            spell_effect: [{}({})],
        )"#,
                effect.to_lowercase(),
                effect,
                effect,
                if effect == "SkipNextUntap" {
                    "target: (kind: AnyPlayer)"
                } else {
                    "subject: Chosen((kind: AnyPlayer))"
                }
            );
            let err = CardRegistry::from_chunks(&[&bad]).unwrap_err();
            assert!(
                matches!(err, RegistryError::InvalidCard { .. }),
                "{effect} at a player should be rejected, got {err:?}"
            );
        }
    }

    #[test]
    fn load_rejects_invalid_intervening_game_condition() {
        let bad = r#"(
            id: "bad_end_step_condition",
            name: "Bad End Step Condition",
            face_id: "bad_end_step_condition",
            mana_cost: "{G}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                ability_id: "triggered_01",
                presentation: Fallback,
                trigger: AtBeginningOfEndStep(player: Controller),
                intervening_if: Some(CreatureDeathsThisTurn(min: None, max: None)),
                effect: [Draw(count: 1)],
            )],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(
            matches!(err, RegistryError::InvalidCard { ref reason, .. } if reason.contains("requires at least one"))
        );
    }

    #[test]
    fn load_rejects_source_untap_in_a_spell_definition() {
        let bad = r#"(
            id: "bad_source_untap",
            name: "Bad Source Untap",
            face_id: "bad_source_untap",
            mana_cost: "{G}",
            types: ["Instant"],
            spell_effect: [Untap(subject: Source)],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(
            matches!(err, RegistryError::InvalidCard { ref reason, .. } if reason.contains("source-bound effects"))
        );
    }

    /// `LoseLife(amount: TargetManaValue)` reads a sibling effect's target, so a list without
    /// an object-targeting effect is invalid data — it would silently resolve to 0 life.
    #[test]
    fn load_rejects_target_mana_value_without_an_object_target() {
        let bad = r#"(
            id: "bad_lose_life",
            name: "Bad Lose Life",
            face_id: "bad_lose_life",
            mana_cost: "{B}",
            types: ["Sorcery"],
            spell_effect: [LoseLife(amount: TargetManaValue)],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(matches!(err, RegistryError::InvalidCard { ref id, .. } if id == "bad_lose_life"));

        // A player target is not enough either — players have no mana value.
        let bad_player_target = r#"(
            id: "bad_lose_life_player",
            name: "Bad Lose Life Player",
            face_id: "bad_lose_life_player",
            mana_cost: "{B}",
            types: ["Sorcery"],
            spell_effect: [
                MillTargetPlayer(count: 2, target: (kind: AnyPlayer)),
                LoseLife(amount: TargetManaValue),
            ],
        )"#;
        let err = CardRegistry::from_chunks(&[bad_player_target]).unwrap_err();
        assert!(
            matches!(err, RegistryError::InvalidCard { ref id, .. } if id == "bad_lose_life_player")
        );

        // Paired with a graveyard-card target (Reanimate's shape) it loads.
        let good = r#"(
            id: "good_lose_life",
            name: "Good Lose Life",
            face_id: "good_lose_life",
            mana_cost: "{B}",
            types: ["Sorcery"],
            spell_effect: [
                MoveGraveyardCards(
                    filter: (owner: AnyPlayer, card_type: Some(Creature)),
                    destination: Battlefield(tapped: false),
                ),
                LoseLife(amount: TargetManaValue),
            ],
        )"#;
        assert!(CardRegistry::from_chunks(&[good]).is_ok());

        // A fixed amount never needs a target.
        let fixed = r#"(
            id: "fixed_lose_life",
            name: "Fixed Lose Life",
            face_id: "fixed_lose_life",
            mana_cost: "{B}",
            types: ["Sorcery"],
            spell_effect: [LoseLife(amount: Fixed(2))],
        )"#;
        assert!(CardRegistry::from_chunks(&[fixed]).is_ok());
    }

    #[test]
    fn load_rejects_invalid_triggered_ability_effect() {
        let bad = r#"(
            id: "bad_trigger",
            name: "Bad Trigger",
            face_id: "bad_trigger",
            mana_cost: "{G}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [
                (
                    ability_id: "triggered_01",
                    presentation: Fallback,
                    trigger: WhenSelfEntersBattlefield,
                    effect: [TargetPlayerGainsLife(amount: 3, target: (kind: Creature))],
                ),
            ],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(matches!(err, RegistryError::InvalidCard { ref id, .. } if id == "bad_trigger"));
    }

    #[test]
    fn load_rejects_attack_recipients_on_triggers_without_combat_context() {
        let bad = r#"(
            id: "bad_defender_recipient",
            name: "Bad Defender Recipient",
            face_id: "bad_defender_recipient",
            mana_cost: "{R}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                ability_id: "triggered_01",
                presentation: Fallback,
                trigger: WhenSelfEntersBattlefield,
                effect: [DamagePlayer(amount: 1, who: DefendingPlayer)],
            )],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(matches!(
            err,
            RegistryError::InvalidCard { ref id, ref reason }
                if id == "bad_defender_recipient" && reason.contains("defending-player")
        ));
    }

    /// CR 608.2: an ability may carry several effects, resolved in written order — the same
    /// shape as a spell's `spell_effect`. Both ability kinds accept a list, and the per-effect
    /// context validation still runs on every element (the `Source`-in-a-spell rejection below
    /// proves the per-element half).
    #[test]
    fn abilities_accept_multiple_effects() {
        let card = r#"(
            id: "multi_effect",
            name: "Multi Effect",
            face_id: "multi_effect",
            mana_cost: "{B}",
            types: ["Enchantment"],
            activated_abilities: [
                (
                    ability_id: "activated_01",
                    presentation: Fallback,
                    costs: [Mana("{1}")],
                    effect: [Draw(count: 1), LoseLife(amount: Fixed(1))],
                ),
            ],
            triggered_abilities: [
                (
                    ability_id: "triggered_01",
                    presentation: Fallback,
                    trigger: WhenSelfEntersBattlefield,
                    effect: [GainLife(amount: 2), Draw(count: 1)],
                ),
            ],
        )"#;
        let reg = CardRegistry::from_chunks(&[card]).expect("multi-effect abilities load");
        let def = reg.get("multi_effect").expect("card present");
        let face = def.primary_face();
        assert_eq!(face.activated_abilities[0].effect.len(), 2);
        assert_eq!(face.triggered_abilities[0].effect.len(), 2);
        // Order is the authored order — the engine resolves the list front to back.
        assert!(matches!(
            face.triggered_abilities[0].effect[0],
            SpellEffectKind::GainLife { .. }
        ));
    }

    /// A many-effect ability is *not* a mana ability (CR 605.1a) even if one of its effects
    /// produces mana: the fast no-stack path is reserved for the sole-effect case.
    #[test]
    fn mana_options_requires_produce_mana_to_be_the_only_effect() {
        let card = r#"(
            id: "impure_mana",
            name: "Impure Mana",
            face_id: "impure_mana",
            mana_cost: "{1}",
            types: ["Artifact"],
            activated_abilities: [
                (
                    ability_id: "activated_01",
                    presentation: Fallback,
                    costs: [Tap],
                    effect: [ProduceMana(options: [(c: 1)]), LoseLife(amount: Fixed(1))],
                ),
                (
                    ability_id: "activated_02",
                    presentation: Fallback,
                    costs: [Tap],
                    effect: [ProduceMana(options: [(c: 1)])],
                ),
            ],
        )"#;
        let reg = CardRegistry::from_chunks(&[card]).expect("card loads");
        let face = reg.get("impure_mana").unwrap().primary_face();
        assert!(face.activated_abilities[0].mana_options().is_none());
        assert!(face.activated_abilities[1].mana_options().is_some());
    }

    /// A self-pump trigger (the replacement for the old `TriggeredEffect::PumpSelf`) loads,
    /// while the same `Source` subject in a spell's effect list is rejected at load.
    #[test]
    fn self_pump_trigger_loads_but_self_in_spell_rejected() {
        let good = r#"(
            id: "self_pumper",
            name: "Self Pumper",
            face_id: "self_pumper",
            mana_cost: "{G}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [
                (
                    ability_id: "triggered_01",
                    presentation: Fallback,
                    trigger: AtBeginningOfUpkeep(player: Controller),
                    effect: [PumpTarget(power: 1, toughness: 1, subject: Source)],
                ),
            ],
        )"#;
        assert!(CardRegistry::from_chunks(&[good]).is_ok());

        let bad = r#"(
            id: "self_spell",
            name: "Self Spell",
            face_id: "self_spell",
            mana_cost: "{G}",
            types: ["Instant"],
            spell_effect: [PumpTarget(power: 1, toughness: 1, subject: Source)],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(matches!(err, RegistryError::InvalidCard { ref id, .. } if id == "self_spell"));
    }

    /// Authoring convention (not a wire contract): every card's id is the slug of its name.
    /// Catches id/name typos in current and future RON; Phase 6 codegen reuses `slugify`.
    #[test]
    fn card_ids_follow_slug_convention() {
        let reg = CardRegistry::from_embedded().unwrap();
        for def in reg.definitions() {
            assert_eq!(
                def.id,
                crate::slug::slugify(&def.name),
                "card '{}': id does not match slugify(name)",
                def.name
            );
        }
    }

    /// CR 709/712/715: a multi-face card loads with a `faces` vec, resolves to the card id by
    /// either its whole-card `//` name or either face name, and each face exposes its own
    /// characteristics. The slug invariant (tested above) covers the `//` whole-card name.
    #[test]
    fn multiface_card_loads_and_resolves_by_face_name() {
        let reg = CardRegistry::from_embedded().unwrap();
        let def = reg.get("fire_ice").expect("fire_ice loaded");
        assert_eq!(def.face_count(), 2);
        assert!(def.is_multiface());
        // Whole-card name and both face names all resolve to the one card id.
        assert_eq!(reg.id_for_name("Fire // Ice"), Some("fire_ice"));
        assert_eq!(reg.id_for_name("Fire"), Some("fire_ice"));
        assert_eq!(reg.id_for_name("Ice"), Some("fire_ice"));
        // Each face carries its own cost/types.
        let fire = def.face(0).unwrap();
        let ice = def.face(1).unwrap();
        assert_eq!(fire.name, "Fire");
        assert_eq!(ice.name, "Ice");
        assert!(fire.is_instant && ice.is_instant);
        assert!(!fire.is_permanent() && !ice.is_permanent());
    }

    /// The flat authoring schema (`mana_cost`/`types`/… at the top level) is normalized into
    /// `faces[0]` at load, so the runtime definition is faces-only for single-face cards too.
    #[test]
    fn flat_authoring_becomes_a_single_face() {
        let reg = CardRegistry::from_embedded().unwrap();
        let def = reg.get("grizzly_bears").expect("grizzly_bears loaded");
        assert_eq!(def.face_count(), 1);
        assert!(!def.is_multiface());
        let face = def.primary_face();
        assert_eq!(face.name, def.name);
        assert_eq!(face.mana_cost.to_string(), "{1}{G}");
        assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
        assert!(face.is_creature && face.is_permanent());
    }

    /// The layout and the authored `faces` list must agree: a multi-face layout has to author
    /// faces, and a `Normal` card must not (its lone face is the flat fields).
    #[test]
    fn load_rejects_layout_and_faces_disagreement() {
        let faceless_split = r#"(
            id: "faceless_split",
            name: "Faceless Split",
            face_id: "faceless_split",
            layout: Split,
            mana_cost: "{R}",
            types: ["Instant"],
        )"#;
        let err = CardRegistry::from_chunks(&[faceless_split]).unwrap_err();
        match err {
            RegistryError::InvalidCard { id, reason } => {
                assert_eq!(id, "faceless_split");
                assert!(reason.contains("requires an authored `faces` list"));
            }
            other => panic!("expected InvalidCard, got {other:?}"),
        }

        let normal_with_faces = r#"(
            id: "normal_with_faces",
            name: "Normal With Faces",
            faces: [
                (name: "A", face_id: "a", mana_cost: "{R}", types: ["Instant"]),
                (name: "B", face_id: "b", mana_cost: "{U}", types: ["Instant"]),
            ],
        )"#;
        let err = CardRegistry::from_chunks(&[normal_with_faces]).unwrap_err();
        match err {
            RegistryError::InvalidCard { id, reason } => {
                assert_eq!(id, "normal_with_faces");
                assert!(reason.contains("Normal-layout card"));
            }
            other => panic!("expected InvalidCard, got {other:?}"),
        }
    }

    #[test]
    fn load_rejects_invalid_room_shape() {
        let one_door = r#"(
            id: "one_door",
            name: "One Door",
            layout: Room,
            faces: [(name: "Only Door", face_id: "only_door", mana_cost: "{2}", types: ["Enchantment", "Room"])],
        )"#;
        let err = CardRegistry::from_chunks(&[one_door]).unwrap_err();
        assert!(
            matches!(err, RegistryError::InvalidCard { reason, .. } if reason.contains("exactly two doors"))
        );

        let mismatched_types = r#"(
            id: "mismatched_room",
            name: "Mismatched Room",
            layout: Room,
            faces: [
                (name: "Left", face_id: "left", mana_cost: "{2}", types: ["Enchantment", "Room"]),
                (name: "Right", face_id: "right", mana_cost: "{3}", types: ["Artifact"]),
            ],
        )"#;
        let err = CardRegistry::from_chunks(&[mismatched_types]).unwrap_err();
        assert!(
            matches!(err, RegistryError::InvalidCard { reason, .. } if reason.contains("shared type line"))
        );
    }

    #[test]
    fn id_for_name_normalizes_trim_and_case() {
        let reg = CardRegistry::from_embedded().unwrap();
        assert_eq!(reg.id_for_name("Lightning Bolt"), Some("lightning_bolt"));
        assert_eq!(reg.id_for_name("  lightning BOLT "), Some("lightning_bolt"));
        assert_eq!(reg.id_for_name("Pharika's Chosen"), Some("pharikas_chosen"));
        assert_eq!(reg.id_for_name("Black Lotus"), None);
    }

    #[test]
    fn load_rejects_duplicate_name() {
        let a = r#"(
            id: "dupe_a",
            name: "Dupe",
            face_id: "dupe",
            mana_cost: "",
            types: ["Land"],
        )"#;
        let b = r#"(
            id: "dupe_b",
            name: " DUPE ",
            face_id: "dupe",
            mana_cost: "",
            types: ["Land"],
        )"#;
        let err = CardRegistry::from_chunks(&[a, b]).unwrap_err();
        match err {
            RegistryError::InvalidCard { id, reason } => {
                assert_eq!(id, "dupe_b");
                assert!(reason.contains("duplicate name"));
            }
            other => panic!("expected InvalidCard, got {other:?}"),
        }
    }

    #[test]
    fn load_rejects_duplicate_id() {
        let card = r#"(
            id: "dupe",
            name: "Dupe",
            face_id: "dupe",
            mana_cost: "",
            types: ["Land"],
        )"#;
        let err = CardRegistry::from_chunks(&[card, card]).unwrap_err();
        match err {
            RegistryError::InvalidCard { id, reason } => {
                assert_eq!(id, "dupe");
                assert!(reason.contains("duplicate"));
            }
            other => panic!("expected InvalidCard, got {other:?}"),
        }
    }

    #[test]
    fn tokens_load_into_separate_namespace_with_explicit_colors() {
        use crate::primitives::Color;
        let reg = CardRegistry::from_embedded().unwrap();
        // A token resolves through get() but is flagged as a token and is not a deck card.
        let soldier = reg.get("soldier_w_1_1").expect("soldier token");
        assert!(reg.is_token("soldier_w_1_1"));
        assert!(!reg.is_token("lightning_bolt"));
        let face = soldier.primary_face();
        assert!(face.is_creature);
        assert_eq!((face.power, face.toughness), (Some(1), Some(1)));
        // CR 111.4: color comes from the creating effect, not a (nonexistent) mana cost.
        assert_eq!(face.colors(), vec![Color::White]);
        // Tokens never appear in the name index or the implemented-card iterator.
        assert_eq!(reg.id_for_name("Soldier"), None);
        assert!(reg.definitions().all(|d| !reg.is_token(&d.id)));
    }

    #[test]
    fn token_ids_extend_name_slug() {
        // A token's name is just its subtype ("Soldier"), but its identity (CR 111.4) is the full
        // characteristic tuple, so several distinct tokens can share a name. Each therefore gets a
        // descriptive id of the form `<name-slug>[_<characteristics...>]` (e.g. `soldier_w_1_1`,
        // `soldier_w_1_1_lifelink`). We can't require id == slugify(name) (that allows only one
        // token per name); instead require slugify(name) to be the id's leading segment, keeping
        // the id traceable to the name. Uniqueness is enforced at load (duplicate token id error).
        let reg = CardRegistry::from_embedded().unwrap();
        for (id, def) in &reg.tokens {
            let slug = crate::slug::slugify(&def.name);
            let ok = *id == slug
                || id
                    .strip_prefix(&slug)
                    .is_some_and(|rest| rest.starts_with('_'));
            assert!(
                ok,
                "token '{}': id '{id}' must be '{slug}' or start with '{slug}_'",
                def.name
            );
        }
    }

    #[test]
    fn create_tokens_referencing_unknown_token_rejected() {
        let bad = r#"(
            id: "bad_maker",
            name: "Bad Maker",
            face_id: "bad_maker",
            mana_cost: "{W}",
            types: ["Sorcery"],
            spell_effect: [CreateTokens(token: "no_such_token", count: 1)],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(matches!(err, RegistryError::InvalidCard { ref id, .. } if id == "bad_maker"));
    }

    #[test]
    fn resolution_branch_requirements_reject_unsupported_and_ambiguous_shapes() {
        let unsupported = r#"(
            id: "unsupported_requirement",
            name: "Unsupported Requirement",
            face_id: "unsupported_requirement",
            mana_cost: "{W}",
            types: ["Sorcery"],
            spell_effect: [ChooseResolutionBranch(
                optional: true,
                branches: [(
                    branch_id: "draw",
                    presentation: Fallback,
                    cost: None,
                    requirement: EffectsApplicable,
                    effects: [Draw(count: 1)],
                )],
            )],
        )"#;
        let err = CardRegistry::from_chunks(&[unsupported]).unwrap_err();
        assert!(matches!(
            err,
            RegistryError::InvalidCard { ref reason, .. }
                if reason.contains("EffectsApplicable requires a supported")
        ));

        let multiple_choosers = r#"(
            id: "multiple_choosers",
            name: "Multiple Choosers",
            face_id: "multiple_choosers",
            mana_cost: "{W}",
            types: ["Sorcery"],
            spell_effect: [ChooseResolutionBranch(
                chooser: EachPlayer,
                optional: true,
                branches: [(
                    branch_id: "gain_life",
                    presentation: Fallback,
                    cost: None,
                    effects: [GainLife(amount: 1)],
                )],
            )],
        )"#;
        let err = CardRegistry::from_chunks(&[multiple_choosers]).unwrap_err();
        assert!(matches!(
            err,
            RegistryError::InvalidCard { ref reason, .. }
                if reason.contains("exactly one deciding player")
        ));

        let missing_fallback = r#"(
            id: "missing_fallback",
            name: "Missing Fallback",
            face_id: "missing_fallback",
            mana_cost: "{1}{W}",
            types: ["Creature", "Human"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                ability_id: "triggered_01",
                presentation: Fallback,
                trigger: WhenSelfEntersBattlefield,
                effect: [ChooseResolutionBranch(
                    optional: false,
                    branches: [(
                        branch_id: "put_a_counter_on_this",
                        presentation: Fallback,
                        cost: None,
                        requirement: EffectsApplicable,
                        effects: [PutCounters(
                            counter: PlusOnePlusOne,
                            count: 1,
                            subject: Source,
                        )],
                    )],
                )],
            )],
        )"#;
        let err = CardRegistry::from_chunks(&[missing_fallback]).unwrap_err();
        assert!(matches!(
            err,
            RegistryError::InvalidCard { ref reason, .. }
                if reason.contains("unconditional costless fallback")
        ));
    }

    #[test]
    fn load_rejects_card_with_both_spell_effect_and_custom_effect() {
        // A face may have exactly one resolution owner (tier-1/2 data or tier-3 custom).
        let bad = r#"(
            id: "double_owner",
            name: "Double Owner",
            face_id: "double_owner",
            mana_cost: "{U}",
            types: ["Instant"],
            spell_effect: [Draw(count: 1)],
            custom_effect: "brainstorm",
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        match err {
            RegistryError::InvalidCard { id, reason } => {
                assert_eq!(id, "double_owner");
                assert!(reason.contains("one resolution owner"));
            }
            other => panic!("expected InvalidCard, got {other:?}"),
        }
    }

    #[test]
    fn issue_174_tokens_preserve_and_validate_static_abilities() {
        let token = r#"(id: "faerie_u_1_1_restricted", name: "Faerie", face_id: "faerie", types: ["Creature", "Faerie"],
            colors: [Blue], power: 1, toughness: 1, keywords: [Flying],
            static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: SelfCombatRestriction(restriction: (
                cant_block_creatures_matching: [(kind: Creature, excluded_keywords: [Flying])]
            )))])"#;
        let registry = CardRegistry::from_chunks_and_tokens(&[], &[token]).unwrap();
        assert_eq!(
            registry
                .get("faerie_u_1_1_restricted")
                .unwrap()
                .primary_face()
                .static_abilities
                .len(),
            1
        );
        let invalid = token.replace("kind: Creature", "kind: AnyPlayer");
        assert!(CardRegistry::from_chunks_and_tokens(&[], &[&invalid]).is_err());
    }

    #[test]
    fn issue_174_composable_combat_restrictions_load() {
        let card = r#"(
            id: "combat_predicates", name: "Combat Predicates", face_id: "combat_predicates",
            types: ["Creature"], power: 2, toughness: 2,
            static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: SelfCombatRestriction(
                restriction: (
                    cant_be_blocked_by: [(kind: Creature, permanent_types: [Artifact])],
                    cant_block_creatures_matching: [(kind: Creature, excluded_keywords: [Flying])],
                    minimum_blockers: Some(3), maximum_blockers: Some(4),
                ),
                condition: Some(GraveyardAggregate(owners: Controller, aggregate: CardCount, min: Some(7))),
            ))],
        )"#;
        CardRegistry::from_chunks(&[card])
            .expect("typed static and conditional combat restrictions");
        for (old, new) in [
            ("minimum_blockers: Some(3)", "minimum_blockers: Some(0)"),
            ("maximum_blockers: Some(4)", "maximum_blockers: Some(2)"),
            (
                "kind: Creature, permanent_types",
                "kind: Creature, controller: Opponent, permanent_types",
            ),
            (
                "excluded_keywords: [Flying]",
                "excluded_keywords: [Flying], required_keywords: [Flying]",
            ),
        ] {
            assert!(
                CardRegistry::from_chunks(&[&card.replace(old, new)]).is_err(),
                "invalid authored restriction: {new}"
            );
        }
    }

    #[test]
    fn self_combat_restriction_requires_at_least_one_prohibition() {
        let bad = r#"(
            id: "empty_self_combat_restriction",
            name: "Empty Self Combat Restriction",
            face_id: "empty_self_combat_restriction",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: SelfCombatRestriction())],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(matches!(
            err,
            RegistryError::InvalidCard { ref reason, .. }
                if reason.contains("requires at least one restriction")
        ));
    }

    #[test]
    fn vampire_soulcaller_uses_self_combat_restriction() {
        let registry = CardRegistry::from_embedded().expect("embedded registry");
        let face = registry
            .get("vampire_soulcaller")
            .expect("Vampire Soulcaller")
            .primary_face();
        let [restriction] = face.static_abilities.as_slice() else {
            panic!("expected one static ability");
        };
        assert!(matches!(
            &restriction.definition,
            StaticAbilityDef::SelfCombatRestriction {
                restriction, condition: None,
            } if restriction.cant_block
        ));
    }

    #[test]
    fn modal_spell_deserializes_and_validates() {
        let good = r#"(
            id: "modal_test",
            name: "Modal Test",
            face_id: "modal_test",
            mana_cost: "{W}",
            types: ["Instant"],
            modal_spell: (
                min_modes: 1,
                max_modes: 2,
                modes: [
                    (mode_id: "mode_01", presentation: OracleLines([1]), effects: [GainLife(amount: 3)]),
                    (mode_id: "mode_02", presentation: OracleLines([2]), effects: [Draw(count: 1)]),
                ],
            ),
        )"#;
        let registry = CardRegistry::from_chunks(&[good]).unwrap();
        let modal = registry
            .get("modal_test")
            .unwrap()
            .primary_face()
            .modal_spell
            .as_ref()
            .unwrap();
        assert_eq!((modal.min_modes, modal.max_modes), (1, 2));
        assert_eq!(modal.modes.len(), 2);
    }

    #[test]
    fn modal_modes_require_stable_identity_and_external_presentation() {
        let valid = r#"(
            id: "modal_identity", name: "Modal Identity", face_id: "modal_identity",
            types: ["Instant"],
            modal_spell: (
                min_modes: 1,
                max_modes: 1,
                modes: [
                    (mode_id: "mode_01", presentation: OracleLines([2]), effects: [Draw(count: 1)]),
                    (mode_id: "mode_02", presentation: Fallback, effects: [GainLife(amount: 1)]),
                ],
            ),
        )"#;
        CardRegistry::from_chunks(&[valid])
            .expect("stable modal identity and presentation should load");

        let invalid_modes = [
            r#"(label: "Draw a card", effects: [Draw(count: 1)])"#,
            r#"(mode_id: "mode_01", effects: [Draw(count: 1)])"#,
            r#"(presentation: Fallback, effects: [Draw(count: 1)])"#,
            r#"(mode_id: "Mode-01", presentation: Fallback, effects: [Draw(count: 1)])"#,
            r#"(mode_id: "fallback", presentation: Fallback, effects: [Draw(count: 1)])"#,
            r#"(mode_id: "mode_01", presentation: OracleLines([0]), effects: [Draw(count: 1)])"#,
        ];
        for mode in invalid_modes {
            let card = format!(
                r#"(
                    id: "bad_modal_identity", name: "Bad Modal Identity",
                    face_id: "bad_modal_identity", types: ["Instant"],
                    modal_spell: (min_modes: 1, max_modes: 1, modes: [{mode}]),
                )"#
            );
            assert!(
                CardRegistry::from_chunks(&[&card]).is_err(),
                "must reject {mode}"
            );
        }

        let duplicate = r#"(
            id: "duplicate_modes", name: "Duplicate Modes", face_id: "duplicate_modes",
            types: ["Instant"],
            modal_spell: (
                min_modes: 1,
                max_modes: 1,
                modes: [
                    (mode_id: "mode_01", presentation: Fallback, effects: [Draw(count: 1)]),
                    (mode_id: "mode_01", presentation: Fallback, effects: [GainLife(amount: 1)]),
                ],
            ),
        )"#;
        assert!(CardRegistry::from_chunks(&[duplicate]).is_err());
    }

    #[test]
    fn load_rejects_invalid_modal_spell_definitions() {
        let invalid = [
            r#"(
                id: "bad_bounds",
                name: "Bad Bounds",
                face_id: "bad_bounds",
                types: ["Instant"],
                modal_spell: (
                    min_modes: 2,
                    max_modes: 1,
                    modes: [(mode_id: "mode_01", presentation: Fallback, effects: [Draw(count: 1)])],
                ),
            )"#,
            r#"(
                id: "empty_effects",
                name: "Empty Effects",
                face_id: "empty_effects",
                types: ["Instant"],
                modal_spell: (
                    min_modes: 1,
                    max_modes: 1,
                    modes: [(mode_id: "mode_01", presentation: Fallback, effects: [])],
                ),
            )"#,
        ];
        for bad in invalid {
            assert!(
                matches!(
                    CardRegistry::from_chunks(&[bad]),
                    Err(RegistryError::InvalidCard { .. })
                ),
                "expected invalid modal definition to be rejected"
            );
        }
    }

    #[test]
    fn load_rejects_modal_spell_with_another_resolution_owner() {
        let bad = r#"(
            id: "modal_double_owner",
            name: "Modal Double Owner",
            face_id: "modal_double_owner",
            types: ["Instant"],
            spell_effect: [Draw(count: 1)],
            modal_spell: (
                min_modes: 1,
                max_modes: 1,
                modes: [(mode_id: "mode_01", presentation: Fallback, effects: [GainLife(amount: 3)])],
            ),
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(
            matches!(err, RegistryError::InvalidCard { ref reason, .. } if reason.contains("one resolution owner"))
        );
    }

    #[test]
    fn issue_50_enters_tapped_card_cohort_has_the_shared_data_shape() {
        let reg = CardRegistry::from_embedded().unwrap();
        let gainlands = [
            "blossoming_sands",
            "dismal_backwater",
            "jungle_hollow",
            "rugged_highlands",
            "swiftwater_cliffs",
            "thornwood_falls",
            "tranquil_cove",
            "wind-scarred_crag",
        ];
        let simple_duals = [
            "forsaken_sanctuary",
            "foul_orchard",
            "meandering_river",
            "submerged_boneyard",
            "tranquil_expanse",
            "woodland_stream",
        ];

        for id in gainlands.into_iter().chain(simple_duals) {
            let face = reg.get(id).unwrap().primary_face();
            assert!(face.static_abilities.iter().any(|ability| matches!(
                &ability.definition,
                StaticAbilityDef::EntersTapped {
                    affected: EntersTappedAffected::Self_,
                    ..
                }
            )));
            assert_eq!(face.activated_abilities.len(), 1, "{id}");
            assert_eq!(
                face.activated_abilities[0]
                    .mana_options()
                    .expect("dual-land mana ability")
                    .len(),
                2,
                "{id}"
            );
        }

        for id in gainlands {
            let face = reg.get(id).unwrap().primary_face();
            assert!(
                face.triggered_abilities.iter().any(|ability| {
                    ability.trigger == TriggerCondition::WhenSelfEntersBattlefield
                        && ability.effect
                            == [SpellEffectKind::GainLife {
                                amount: Amount::Fixed(1),
                            }]
                }),
                "{id}"
            );
        }

        let ghoul = reg.get("diregraf_ghoul").unwrap().primary_face();
        assert!(ghoul.static_abilities.iter().any(|ability| matches!(
            &ability.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                ..
            }
        )));
        let orb = reg.get("orb_of_dreams").unwrap().primary_face();
        assert!(orb.static_abilities.iter().any(|ability| matches!(
            &ability.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Permanents,
                ..
            }
        )));
    }

    #[test]
    fn issue_97_entry_replacement_cards_have_exact_shared_data_shapes() {
        let registry = CardRegistry::from_embedded().unwrap();
        let expected_condition = GameCondition::PlayerLifeAggregate {
            players: RelativePlayerSet::All,
            aggregate: PlayerLifeAggregate::Minimum,
            min: Some(14),
            max: None,
        };
        let lands = [
            (
                "abandoned_campground",
                [
                    ManaAmount {
                        w: 1,
                        ..ManaAmount::default()
                    },
                    ManaAmount {
                        u: 1,
                        ..ManaAmount::default()
                    },
                ],
            ),
            (
                "bleeding_woods",
                [
                    ManaAmount {
                        r: 1,
                        ..ManaAmount::default()
                    },
                    ManaAmount {
                        g: 1,
                        ..ManaAmount::default()
                    },
                ],
            ),
            (
                "lakeside_shack",
                [
                    ManaAmount {
                        g: 1,
                        ..ManaAmount::default()
                    },
                    ManaAmount {
                        u: 1,
                        ..ManaAmount::default()
                    },
                ],
            ),
            (
                "murky_sewer",
                [
                    ManaAmount {
                        u: 1,
                        ..ManaAmount::default()
                    },
                    ManaAmount {
                        b: 1,
                        ..ManaAmount::default()
                    },
                ],
            ),
            (
                "neglected_manor",
                [
                    ManaAmount {
                        w: 1,
                        ..ManaAmount::default()
                    },
                    ManaAmount {
                        b: 1,
                        ..ManaAmount::default()
                    },
                ],
            ),
            (
                "razortrap_gorge",
                [
                    ManaAmount {
                        b: 1,
                        ..ManaAmount::default()
                    },
                    ManaAmount {
                        r: 1,
                        ..ManaAmount::default()
                    },
                ],
            ),
            (
                "strangled_cemetery",
                [
                    ManaAmount {
                        b: 1,
                        ..ManaAmount::default()
                    },
                    ManaAmount {
                        g: 1,
                        ..ManaAmount::default()
                    },
                ],
            ),
        ];

        for (id, expected_mana) in lands {
            let face = registry.get(id).unwrap().primary_face();
            assert_eq!(
                face.activated_abilities[0].mana_options().unwrap(),
                &expected_mana,
                "{id} mana options"
            );
            assert!(
                face.static_abilities.iter().any(|ability| matches!(
                    &ability.definition,
                    StaticAbilityDef::EntersTapped {
                        affected: EntersTappedAffected::Self_,
                        condition: Some(condition),
                    } if condition == &expected_condition
                )),
                "{id} condition"
            );
        }

        let globe = registry.get("dragonstorm_globe").unwrap().primary_face();
        assert_eq!(globe.mana_cost.to_string(), "{3}");
        assert_eq!(
            globe.activated_abilities[0].mana_options().unwrap(),
            &[
                ManaAmount {
                    w: 1,
                    ..ManaAmount::default()
                },
                ManaAmount {
                    u: 1,
                    ..ManaAmount::default()
                },
                ManaAmount {
                    b: 1,
                    ..ManaAmount::default()
                },
                ManaAmount {
                    r: 1,
                    ..ManaAmount::default()
                },
                ManaAmount {
                    g: 1,
                    ..ManaAmount::default()
                },
            ]
        );
        assert!(globe.static_abilities.iter().any(|ability| matches!(
            &ability.definition,
            StaticAbilityDef::EntersWithCounters {
                affected: EntersWithCountersAffected::Creatures(CreatureScopeFilter {
                    controller: Some(CreatureScopeController::YouControl),
                    subtype: Some(subtype),
                    ..
                }),
                counter: CounterKind::PlusOnePlusOne,
                amount: Amount::Fixed(1),
                ..
            } if subtype == "Dragon"
        )));
    }

    #[test]
    fn issue_60_end_step_cards_share_the_trigger_and_condition_shape() {
        let registry = CardRegistry::from_embedded().unwrap();
        let death_condition = Some(GameCondition::CreatureDeathsThisTurn {
            min: Some(1),
            max: None,
        });

        let mauler = &registry
            .get("sabertooth_mauler")
            .unwrap()
            .primary_face()
            .triggered_abilities[0];
        assert_eq!(
            mauler.trigger,
            TriggerCondition::AtBeginningOfEndStep {
                player: CastTriggerPlayer::Controller,
            }
        );
        assert_eq!(mauler.intervening_if, death_condition);
        assert_eq!(
            mauler.effect,
            [
                SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                },
                SpellEffectKind::Untap {
                    subject: EffectSubject::Source,
                },
            ]
        );

        let assassins = &registry
            .get("twinblade_assassins")
            .unwrap()
            .primary_face()
            .triggered_abilities[0];
        assert_eq!(assassins.trigger, mauler.trigger);
        assert_eq!(assassins.intervening_if, death_condition);
        assert_eq!(
            assassins.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }]
        );
    }

    #[test]
    fn issue_51_dynamic_entry_counter_cards_share_the_amount_vocabulary() {
        let reg = CardRegistry::from_embedded().unwrap();
        let controlled_creatures = Amount::Count(CountExpression::BattlefieldCreatures {
            filter: BattlefieldCreatureCountFilter {
                controllers: RelativePlayerSet::Controller,
                subtype: None,
                required_keywords: vec![],
                tapped: None,
                requires_any_counter: false,
                required_counter: None,
                exclude_source: false,
            },
        });

        let entry_amount = |id: &str| {
            reg.get(id)
                .unwrap()
                .primary_face()
                .static_abilities
                .iter()
                .find_map(|ability| match &ability.definition {
                    StaticAbilityDef::EntersWithCounters {
                        counter, amount, ..
                    } if *counter == CounterKind::PlusOnePlusOne => Some(amount.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{id} lacks its entry-counter ability"))
        };

        assert_eq!(entry_amount("endless_one"), Amount::X);
        assert_eq!(
            entry_amount("squad_captain"),
            Amount::Count(CountExpression::BattlefieldCreatures {
                filter: BattlefieldCreatureCountFilter {
                    controllers: RelativePlayerSet::Controller,
                    subtype: None,
                    required_keywords: vec![],
                    tapped: None,
                    requires_any_counter: false,
                    required_counter: None,
                    exclude_source: true,
                },
            })
        );
        assert_eq!(
            entry_amount("bloodcrazed_paladin"),
            Amount::Count(CountExpression::CreatureDeathsThisTurn)
        );

        let priest = reg.get("dwarven_priest").unwrap().primary_face();
        assert!(priest.triggered_abilities.iter().any(|ability| {
            ability.trigger == TriggerCondition::WhenSelfEntersBattlefield
                && ability.effect
                    == [SpellEffectKind::GainLife {
                        amount: controlled_creatures.clone(),
                    }]
        }));
    }

    #[test]
    fn authored_color_indicator_overrides_empty_mana_cost() {
        let reg = CardRegistry::from_embedded().unwrap();
        let back = reg
            .get("reckless_waif_merciless_predator")
            .unwrap()
            .face(1)
            .unwrap();
        assert_eq!(back.colors(), vec![crate::primitives::Color::Red]);
    }

    #[test]
    fn face_change_action_must_match_layout() {
        let bad = r#"(
            id: "bad_flip",
            name: "Bad Flip",
            face_id: "bad_flip",
            mana_cost: "{R}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                ability_id: "triggered_01",
                presentation: Fallback,
                trigger: WheneverSelfAttacks(minimum_other_attackers: 0),
                effect: [ChangeSourceFace(action: Flip)],
            )],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(matches!(err, RegistryError::InvalidCard { ref id, .. } if id == "bad_flip"));
    }

    #[test]
    fn load_rejects_malformed_adventure_faces() {
        let invalid = [
            r#"(
                id: "one_face_adventure",
                name: "One Face Adventure",
                layout: Adventure,
                faces: [(name: "Creature", face_id: "creature", mana_cost: "{2}{G}", types: ["Creature"])],
            )"#,
            r#"(
                id: "spell_first_adventure",
                name: "Spell First Adventure",
                layout: Adventure,
                faces: [
                    (name: "Spell", face_id: "spell", mana_cost: "{G}", types: ["Instant"]),
                    (name: "Other Spell", face_id: "other_spell", mana_cost: "{1}{G}", types: ["Sorcery"]),
                ],
            )"#,
            r#"(
                id: "permanent_second_adventure",
                name: "Permanent Second Adventure",
                layout: Adventure,
                faces: [
                    (name: "Creature", face_id: "creature", mana_cost: "{2}{G}", types: ["Creature"]),
                    (name: "Other Creature", face_id: "other_creature", mana_cost: "{1}{G}", types: ["Creature"]),
                ],
            )"#,
        ];
        for bad in invalid {
            assert!(
                matches!(
                    CardRegistry::from_chunks(&[bad]),
                    Err(RegistryError::InvalidCard { .. })
                ),
                "expected malformed Adventure definition to be rejected"
            );
        }
    }

    #[test]
    fn load_rejects_malformed_omen_faces() {
        let invalid = [
            r#"(
                id: "one_face_omen",
                name: "One Face Omen",
                layout: Omen,
                faces: [(name: "Creature", face_id: "creature", mana_cost: "{2}{G}", types: ["Creature"])],
            )"#,
            r#"(
                id: "spell_first_omen",
                name: "Spell First Omen",
                layout: Omen,
                faces: [
                    (name: "Spell", face_id: "spell", mana_cost: "{G}", types: ["Instant"]),
                    (name: "Omen", face_id: "omen", mana_cost: "{1}{G}", types: ["Sorcery", "Omen"]),
                ],
            )"#,
            r#"(
                id: "missing_omen_subtype",
                name: "Missing Omen Subtype",
                layout: Omen,
                faces: [
                    (name: "Creature", face_id: "creature", mana_cost: "{2}{G}", types: ["Creature"]),
                    (name: "Spell", face_id: "spell", mana_cost: "{1}{G}", types: ["Sorcery"]),
                ],
            )"#,
        ];
        for bad in invalid {
            assert!(
                matches!(
                    CardRegistry::from_chunks(&[bad]),
                    Err(RegistryError::InvalidCard { .. })
                ),
                "expected malformed Omen definition to be rejected"
            );
        }
    }

    #[test]
    fn load_rejects_malformed_attachment_definitions() {
        let invalid = [
            r#"(
                id: "aura_without_enchant",
                name: "Aura Without Enchant",
                face_id: "aura_without_enchant",
                mana_cost: "{W}",
                types: ["Enchantment", "Aura"],
                static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: AttachedModifier(keywords: [Flying]))],
            )"#,
            r#"(
                id: "instant_aura_attach",
                name: "Instant Aura Attach",
                face_id: "instant_aura_attach",
                mana_cost: "{W}",
                types: ["Instant"],
                spell_effect: [AuraAttach(target: (kind: Creature))],
            )"#,
            r#"(
                id: "ordinary_enchantment_modifier",
                name: "Ordinary Enchantment Modifier",
                face_id: "ordinary_enchantment_modifier",
                mana_cost: "{W}",
                types: ["Enchantment"],
                static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: AttachedModifier(keywords: [Flying]))],
            )"#,
            r#"(
                id: "empty_attachment_modifier",
                name: "Empty Attachment Modifier",
                face_id: "empty_attachment_modifier",
                mana_cost: "{1}",
                types: ["Artifact", "Equipment"],
                static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: AttachedModifier())],
            )"#,
            r#"(
                id: "ordinary_enchantment_attached_subject",
                name: "Ordinary Enchantment Attached Subject",
                face_id: "ordinary_enchantment_attached_subject",
                mana_cost: "{1}{U}",
                types: ["Enchantment"],
                triggered_abilities: [(
                    ability_id: "triggered_01",
                    presentation: Fallback,
                    trigger: WhenSelfEntersBattlefield,
                    effect: [Tap(subject: AttachedObject)],
                )],
            )"#,
            r#"(
                id: "player_aura_attached_object",
                name: "Player Aura Attached Object",
                face_id: "player_aura_attached_object",
                mana_cost: "{1}{U}",
                types: ["Enchantment", "Aura"],
                spell_effect: [AuraAttach(target: (kind: AnyPlayer))],
                triggered_abilities: [(
                    ability_id: "triggered_01",
                    presentation: Fallback,
                    trigger: WhenSelfEntersBattlefield,
                    effect: [Tap(subject: AttachedObject)],
                )],
            )"#,
        ];
        for bad in invalid {
            assert!(
                matches!(
                    CardRegistry::from_chunks(&[bad]),
                    Err(RegistryError::InvalidCard { .. })
                ),
                "expected malformed attachment definition to be rejected"
            );
        }
    }

    #[test]
    fn issue_216_accepts_typed_attached_combat_restrictions() {
        let card = r#"(
            id: "attached_blocking_restriction",
            name: "Attached Blocking Restriction",
            face_id: "attached_blocking_restriction",
            mana_cost: "{G}",
            types: ["Enchantment", "Aura"],
            spell_effect: [AuraAttach(target: (kind: Creature))],
            static_abilities: [(
                ability_id: "static_01",
                presentation: Fallback,
                definition: AttachedModifier(
                    restriction: (maximum_blockers: Some(1)),
                ),
            )],
        )"#;

        let registry = CardRegistry::from_chunks(&[card])
            .expect("a typed combat restriction is valid on an attached modifier");
        let restriction = match &registry
            .get("attached_blocking_restriction")
            .expect("test card")
            .primary_face()
            .static_abilities[0]
            .definition
        {
            StaticAbilityDef::AttachedModifier { restriction, .. } => restriction,
            other => panic!("expected attached modifier, got {other:?}"),
        };
        assert_eq!(restriction.maximum_blockers, Some(1));

        for invalid in [
            card.replace(
                "restriction: (maximum_blockers: Some(1)),",
                "restriction: (minimum_blockers: Some(2), maximum_blockers: Some(1)),",
            ),
            card.replace(
                "restriction: (maximum_blockers: Some(1)),",
                "condition: Some(ActivePlayer(players: Controller)), restriction: (maximum_blockers: Some(1)),",
            ),
            card.replace(
                "restriction: (maximum_blockers: Some(1)),",
                "cant_attack: true,",
            ),
        ] {
            assert!(
                CardRegistry::from_chunks(&[&invalid]).is_err(),
                "malformed attached combat restriction must fail closed"
            );
        }
    }

    #[test]
    fn load_rejects_malformed_type_additions() {
        let invalid = [
            r#"(
                id: "duplicate_added_card_type",
                name: "Duplicate Added Card Type",
                face_id: "duplicate_added_card_type",
                mana_cost: "{1}",
                types: ["Artifact"],
                activated_abilities: [(
                    ability_id: "activated_01",
                    presentation: Fallback,
                    costs: [Tap],
                    effect: [AddTypes(
                        subject: Chosen((kind: AnyPermanent)),
                        addition: (card_types: [Artifact, Artifact]),
                    )],
                )],
            )"#,
            r#"(
                id: "duplicate_added_creature_type",
                name: "Duplicate Added Creature Type",
                face_id: "duplicate_added_creature_type",
                mana_cost: "{W}",
                types: ["Enchantment", "Aura"],
                spell_effect: [AuraAttach(target: (kind: Creature))],
                static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: AttachedModifier(
                    add_types: (creature_types: ["Knight", "Knight"]),
                ))],
            )"#,
            r#"(
                id: "blank_added_creature_type",
                name: "Blank Added Creature Type",
                face_id: "blank_added_creature_type",
                mana_cost: "{W}",
                types: ["Enchantment", "Aura"],
                spell_effect: [AuraAttach(target: (kind: Creature))],
                static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: AttachedModifier(
                    add_types: (creature_types: [" "]),
                ))],
            )"#,
            r#"(
                id: "empty_type_addition",
                name: "Empty Type Addition",
                face_id: "empty_type_addition",
                mana_cost: "{1}",
                types: ["Artifact"],
                activated_abilities: [(
                    ability_id: "activated_01",
                    presentation: Fallback,
                    costs: [Tap],
                    effect: [AddTypes(
                        subject: Chosen((kind: AnyPermanent)),
                        addition: (),
                    )],
                )],
            )"#,
            r#"(
                id: "player_type_addition",
                name: "Player Type Addition",
                face_id: "player_type_addition",
                mana_cost: "{1}",
                types: ["Artifact"],
                activated_abilities: [(
                    ability_id: "activated_01",
                    presentation: Fallback,
                    costs: [Tap],
                    effect: [AddTypes(
                        subject: Chosen((kind: AnyPlayer)),
                        addition: (card_types: [Artifact]),
                    )],
                )],
            )"#,
            r#"(
                id: "mixed_target_type_addition",
                name: "Mixed Target Type Addition",
                face_id: "mixed_target_type_addition",
                mana_cost: "{1}",
                types: ["Artifact"],
                activated_abilities: [(
                    ability_id: "activated_01",
                    presentation: Fallback,
                    costs: [Tap],
                    effect: [AddTypes(
                        subject: Chosen((kind: AnyTarget)),
                        addition: (card_types: [Artifact]),
                    )],
                )],
            )"#,
            r#"(
                id: "subtype_on_noncreature",
                name: "Subtype On Noncreature",
                face_id: "subtype_on_noncreature",
                mana_cost: "{1}",
                types: ["Artifact"],
                activated_abilities: [(
                    ability_id: "activated_01",
                    presentation: Fallback,
                    costs: [Tap],
                    effect: [AddTypes(
                        subject: Chosen((kind: AnyPermanent)),
                        addition: (creature_types: ["Knight"]),
                    )],
                )],
            )"#,
        ];
        for bad in invalid {
            assert!(
                matches!(
                    CardRegistry::from_chunks(&[bad]),
                    Err(RegistryError::InvalidCard { .. })
                ),
                "expected malformed type addition to be rejected"
            );
        }
    }

    #[test]
    fn source_mana_cost_reduction_rejects_flexible_source_and_ability_symbols() {
        let template = r#"(
            id: "power_up_validation",
            name: "Power Up Validation",
            face_id: "power_up_validation",
            mana_cost: "SOURCE_COST",
            types: ["Creature", "Robot"],
            power: 2,
            toughness: 2,
            activated_abilities: [(
                ability_id: "activated_01",
                presentation: Fallback,
                costs: [Mana("ABILITY_COST")],
                cost_modifiers: [ConditionalSourceManaCostReduction(condition: PermanentsEnteredThisTurn(
                    controllers: All,
                    filter: (source_only: true),
                    min: Some(1),
                ))],
                effect: [GainLife(amount: 1)],
            )],
        )"#;
        for (source, ability) in [("{W/U}", "{3}"), ("{2}", "{2/W}"), ("{X}", "{3}")] {
            let card = template
                .replace("SOURCE_COST", source)
                .replace("ABILITY_COST", ability);
            assert!(
                matches!(
                    CardRegistry::from_chunks(&[&card]),
                    Err(RegistryError::InvalidCard { ref reason, .. })
                        if reason.contains("source mana cost reduction")
                ),
                "must reject source {source} with ability {ability}"
            );
        }
    }

    #[test]
    fn issue_183_power_up_cards_and_tokens_have_complete_registry_shapes() {
        let registry = CardRegistry::global();
        for (id, source_cost, activation_cost) in [
            ("ninja_of_the_hand", "{2}{B}", "{4}{B}"),
            ("ultron_drone", "{3}", "{6}"),
            ("hercules,_prince_of_power", "{2}{G}", "{4}{G}"),
            ("white_tiger,_ava_ayala", "{1}{G}", "{5}{G}"),
            ("viv_vision,_teen_synthezoid", "{3}", "{7}"),
        ] {
            let face = registry
                .get(id)
                .unwrap_or_else(|| panic!("missing {id}"))
                .primary_face();
            assert_eq!(face.mana_cost.to_string(), source_cost, "{id}");
            let [ability] = face.activated_abilities.as_slice() else {
                panic!("{id} must have one power-up ability")
            };
            assert!(matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(cost)] if cost.to_string() == activation_cost
            ));
            assert!(matches!(
                ability.cost_modifiers.as_slice(),
                [ActivatedCostModifier::ConditionalSourceManaCostReduction { .. }]
            ));
            assert!(matches!(
                ability.activation_limit,
                Some(crate::primitives::ActivationLimit::PerObject { max_activations: 1 })
            ));
        }

        let robot = registry
            .get("robot_villain_c_2_2")
            .expect("Robot Villain token");
        assert!(robot.primary_face().has_subtype("Robot"));
        assert!(robot.primary_face().has_subtype("Villain"));
        let tiger = registry.get("the_tiger_god").expect("The Tiger God token");
        assert!(tiger.primary_face().is_legendary);
        assert!(tiger.primary_face().has_subtype("Cat"));
        assert!(tiger.primary_face().has_subtype("God"));

        assert!(
            registry.get("loki,_god_of_stories").is_none(),
            "Loki remains unregistered until its delayed copy-next-spell ability is supported"
        );
    }

    #[test]
    fn issue_113_rejects_malformed_characteristic_replacements() {
        let invalid = [
            r#"(
                id: "empty_replacement_name",
                name: "Empty Replacement Name",
                face_id: "empty_replacement_name",
                mana_cost: "{U}",
                types: ["Enchantment", "Aura"],
                spell_effect: [AuraAttach(target: (kind: Creature))],
                static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: AttachedModifier(set_name: Some(" ")))],
            )"#,
            r#"(
                id: "duplicate_replacement_color",
                name: "Duplicate Replacement Color",
                face_id: "duplicate_replacement_color",
                mana_cost: "{U}",
                types: ["Enchantment", "Aura"],
                spell_effect: [AuraAttach(target: (kind: Creature))],
                static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: AttachedModifier(set_colors: Some([White, White])))],
            )"#,
            r#"(
                id: "conflicting_type_operations",
                name: "Conflicting Type Operations",
                face_id: "conflicting_type_operations",
                mana_cost: "{U}",
                types: ["Enchantment", "Aura"],
                spell_effect: [AuraAttach(target: (kind: Creature))],
                static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: AttachedModifier(
                    add_types: (card_types: [Artifact]),
                    set_types: Some((card_types: [Creature])),
                ))],
            )"#,
            r#"(
                id: "creature_subtype_without_creature_type",
                name: "Creature Subtype Without Creature Type",
                face_id: "creature_subtype_without_creature_type",
                mana_cost: "{U}",
                types: ["Enchantment", "Aura"],
                spell_effect: [AuraAttach(target: (kind: Creature))],
                static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: AttachedModifier(set_types: Some(
                    (card_types: [Artifact], creature_types: ["Citizen"])
                )))],
            )"#,
        ];

        for card in invalid {
            assert!(
                matches!(
                    CardRegistry::from_chunks(&[card]),
                    Err(RegistryError::InvalidCard { .. })
                ),
                "expected malformed characteristic replacement to be rejected"
            );
        }
    }

    #[test]
    fn issue_173_rejects_invalid_snapshot_references_and_definitions() {
        for fields in [
            "cast_conditions: [ActivePlayer(players: Controller)], spell_effect: [GainLife(amount: Conditional(condition: CastSnapshot(index: 1), when_true: 4, otherwise: 2))]",
            "cast_conditions: [CastSnapshot(index: 0)], spell_effect: [GainLife(amount: 2)]",
            "cast_conditions: [CreatureDeathsThisTurn(min: None, max: None)], spell_effect: [GainLife(amount: 2)]",
            "cast_conditions: [ActivePlayer(players: Controller)], modal_spell: (min_modes: 1, max_modes: 1, modes: [(mode_id: \"mode_01\", presentation: Fallback, effects: [GainLife(amount: Conditional(condition: CastSnapshot(index: 1), when_true: 4, otherwise: 2))])])",
            "spell_effect: [ChooseResolutionBranch(selection: FirstApplicable, branches: [(branch_id: \"bonus\", presentation: Fallback, cost: None, requirement: GameCondition(CastSnapshot(index: 0)), effects: [GainLife(amount: 4)]), (branch_id: \"fallback_branch\", presentation: Fallback, cost: None, requirement: Always, effects: [])])]",
        ] {
            let card = format!("(id: \"snapshot_test\", name: \"Snapshot Test\", types: [\"Instant\"], {fields})");
            assert!(matches!(CardRegistry::from_chunks(&[&card]), Err(RegistryError::InvalidCard { .. })), "must reject: {fields}");
        }
    }

    #[test]
    fn issue_173_rejects_snapshots_outside_spell_resolution() {
        for fields in [
            "cost_modifiers: [ConditionalGenericReduction(amount: 1, condition: CastSnapshot(index: 0))]",
            "static_abilities: [(ability_id: \"static_01\", presentation: Fallback, definition: ConditionalSelfModifier(condition: CastSnapshot(index: 0), delta_power: 1))]",
            "static_abilities: [(ability_id: \"static_01\", presentation: Fallback, definition: EntersWithCounters(counter: PlusOnePlusOne, amount: Conditional(condition: CastSnapshot(index: 0), when_true: 4, otherwise: 2)))]",
            "activated_abilities: [(ability_id: \"activated_01\", presentation: Fallback, costs: [], effect: [GainLife(amount: Conditional(condition: CastSnapshot(index: 0), when_true: 4, otherwise: 2))])]",
            "activated_abilities: [(ability_id: \"activated_01\", presentation: Fallback, costs: [], conditions: [CastSnapshot(index: 0)], effect: [GainLife(amount: 1)])]",
            "triggered_abilities: [(ability_id: \"triggered_01\", presentation: Fallback, trigger: WhenSelfEntersBattlefield, effect: [GainLife(amount: Conditional(condition: CastSnapshot(index: 0), when_true: 4, otherwise: 2))])]",
            "triggered_abilities: [(ability_id: \"triggered_01\", presentation: Fallback, trigger: WhenSelfEntersBattlefield, intervening_if: Some(CastSnapshot(index: 0)), effect: [GainLife(amount: 1)])]",
            "triggered_abilities: [(ability_id: \"triggered_01\", presentation: Fallback, trigger: WheneverSelfAttacks(minimum_other_attackers: 0), effect: [DamageAttackedPlayerOrPlaneswalker(amount: Conditional(condition: CastSnapshot(index: 0), when_true: 4, otherwise: 2))])]",
            "spell_effect: [GrantTriggeredAbility(subject: Chosen((kind: Creature)), ability: (ability_id: \"triggered_01\", presentation: Fallback, trigger: WhenSelfDies, effect: [GainLife(amount: Conditional(condition: CastSnapshot(index: 0), when_true: 4, otherwise: 2))]))]",
        ] {
            let card = format!("(id: \"snapshot_test\", name: \"Snapshot Test\", face_id: \"snapshot_test\", types: [\"Creature\"], power: 1, toughness: 1, cast_conditions: [ActivePlayer(players: Controller)], {fields})");
            assert!(matches!(CardRegistry::from_chunks(&[&card]), Err(RegistryError::InvalidCard { .. })), "must reject: {fields}");
        }
    }

    #[test]
    fn content_hash_is_stable_and_well_formed() {
        let h = CardRegistry::content_hash();
        assert_eq!(h.len(), 16, "expected 16-char hex digest");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        // Deterministic within a build (same embedded data → same digest).
        assert_eq!(h, CardRegistry::content_hash());
    }

    #[test]
    fn generated_presentation_catalog_covers_normal_and_multiface_cards() {
        let registry = CardRegistry::from_embedded().expect("embedded registry");
        for (card_id, face_id, card_name, face_name) in [
            (
                "grow_from_the_ashes",
                "grow_from_the_ashes",
                "Grow from the Ashes",
                "Grow from the Ashes",
            ),
            ("fire_ice", "fire", "Fire // Ice", "Fire"),
            ("fire_ice", "ice", "Fire // Ice", "Ice"),
        ] {
            let metadata = registry
                .presentation_face(card_id, face_id)
                .unwrap_or_else(|| panic!("missing {card_id}/{face_id}"));
            assert_eq!(metadata.card_name, card_name);
            assert_eq!(metadata.face_name, face_name);
            assert_eq!(metadata.oracle_text_sha256.len(), 64);
            assert!(metadata
                .oracle_text_sha256
                .chars()
                .all(|value| value.is_ascii_hexdigit() && !value.is_ascii_uppercase()));
        }
    }

    #[test]
    fn stable_identity_and_external_presentation_schema_is_fail_closed() {
        let missing_identity = r#"(
            id: "schema_probe", name: "Schema Probe", types: ["Creature"],
            power: 1, toughness: 1,
            triggered_abilities: [(
                trigger: WhenSelfEntersBattlefield,
                effect: [GainLife(amount: 1)],
                text: "When this creature enters, you gain 1 life.",
            )],
        )"#;
        assert!(CardRegistry::from_chunks(&[missing_identity]).is_err());

        let valid_external_mapping = r#"(
            id: "schema_probe", name: "Schema Probe", face_id: "schema_probe",
            types: ["Creature"], power: 1, toughness: 1,
            triggered_abilities: [(
                ability_id: "triggered_01", presentation: OracleLines([1, 3]),
                trigger: WhenSelfEntersBattlefield,
                effect: [GainLife(amount: 1)],
            )],
        )"#;
        CardRegistry::from_chunks(&[valid_external_mapping])
            .expect("stable identity and external Oracle line references should load");

        let valid_fallback = r#"(
            id: "schema_probe", name: "Schema Probe", face_id: "schema_probe",
            types: ["Creature"], power: 1, toughness: 1,
            activated_abilities: [(
                ability_id: "activated_01", presentation: Fallback,
                costs: [], effect: [GainLife(amount: 1)],
            )],
        )"#;
        CardRegistry::from_chunks(&[valid_fallback])
            .expect("explicit fallback should load without external Oracle text");

        for invalid_id in ["", "Upper", "has-dash", "two__underscores", "fallback"] {
            let data = format!(
                r#"(
                    id: "schema_probe", name: "Schema Probe", face_id: "schema_probe",
                    types: ["Creature"], power: 1, toughness: 1,
                    activated_abilities: [(
                        ability_id: "{invalid_id}", presentation: Fallback,
                        costs: [], effect: [GainLife(amount: 1)],
                    )],
                )"#
            );
            assert!(
                CardRegistry::from_chunks(&[&data]).is_err(),
                "{invalid_id:?}"
            );
        }

        let duplicate_siblings = r#"(
            id: "schema_probe", name: "Schema Probe", face_id: "schema_probe",
            types: ["Creature"], power: 1, toughness: 1,
            activated_abilities: [
                (ability_id: "ability_01", presentation: Fallback, costs: [], effect: [GainLife(amount: 1)]),
                (ability_id: "ability_01", presentation: Fallback, costs: [], effect: [GainLife(amount: 2)]),
            ],
        )"#;
        assert!(CardRegistry::from_chunks(&[duplicate_siblings]).is_err());

        let nested_collision = r#"(
            id: "schema_probe", name: "Schema Probe", face_id: "schema_probe",
            types: ["Creature"], power: 1, toughness: 1,
            static_abilities: [(
                ability_id: "static_01", presentation: Fallback,
                definition: AttachedModifier(
                    activated_abilities: [(
                        ability_id: "granted_01", presentation: Fallback,
                        costs: [], effect: [GainLife(amount: 1)],
                    )],
                    triggered_abilities: [(
                        ability_id: "granted_01", presentation: Fallback,
                        trigger: WhenSelfEntersBattlefield, effect: [GainLife(amount: 1)],
                    )],
                ),
            )],
        )"#;
        assert!(CardRegistry::from_chunks(&[nested_collision]).is_err());

        let absent_presentation = r#"(
            id: "schema_probe", name: "Schema Probe", face_id: "schema_probe",
            types: ["Creature"], power: 1, toughness: 1,
            activated_abilities: [(
                ability_id: "activated_01", costs: [], effect: [GainLife(amount: 1)],
            )],
        )"#;
        assert!(CardRegistry::from_chunks(&[absent_presentation]).is_err());

        for invalid in [
            "OracleLines([])",
            "OracleLines([0])",
            "OracleLines([2, 1])",
            "OracleLines([1, 1])",
        ] {
            let data = format!(
                r#"(
                    id: "schema_probe", name: "Schema Probe", face_id: "schema_probe",
                    types: ["Creature"], power: 1, toughness: 1,
                    activated_abilities: [(
                        ability_id: "activated_01", presentation: {invalid},
                        costs: [], effect: [GainLife(amount: 1)],
                    )],
                )"#
            );
            assert!(CardRegistry::from_chunks(&[&data]).is_err(), "{invalid}");
        }

        for obsolete in ["text: \"legacy copy\",", "oracle_text: \"legacy copy\","] {
            let data = format!(
                r#"(
                    id: "schema_probe", name: "Schema Probe", face_id: "schema_probe",
                    types: ["Creature"], power: 1, toughness: 1,
                    activated_abilities: [(
                        ability_id: "activated_01", presentation: Fallback, {obsolete}
                        costs: [], effect: [GainLife(amount: 1)],
                    )],
                )"#
            );
            assert!(CardRegistry::from_chunks(&[&data]).is_err(), "{obsolete}");
        }
    }

    #[test]
    fn issue_147_rejects_partial_conditional_base_pt_and_unbacked_payment_results() {
        let partial_pt = r#"(
            id: "bad_station_pt", name: "Bad Station PT", face_id: "bad_station_pt",
            types: ["Artifact", "Spacecraft"],
            static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: ConditionalSelfModifier(
                condition: SourceCounterCount(counter: Charge, min: Some(4)),
                base_power: Some(2),
            ))],
        )"#;
        let error = CardRegistry::from_chunks(&[partial_pt]).unwrap_err();
        assert!(matches!(
            error,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("base power and toughness must be provided together")
        ));

        let unbacked_payment = r#"(
            id: "bad_station_payment", name: "Bad Station Payment", face_id: "bad_station_payment",
            types: ["Artifact", "Spacecraft"],
            static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: ConditionalSelfModifier(
                condition: SourceCounterCount(counter: Charge, min: Some(4)),
                activated_abilities: [(
                    ability_id: "activated_01", presentation: Fallback, costs: [],
                    effect: [PutCounters(
                        counter: Charge,
                        count: Count(CardResultCharacteristicSum(
                            filter: (source: Payment, action: Tap, players: Controller, card_type: Some(Creature)),
                            characteristic: Power,
                        )),
                        subject: Source,
                    )],
                )],
            ))],
        )"#;
        let error = CardRegistry::from_chunks(&[unbacked_payment]).unwrap_err();
        assert!(matches!(
            error,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("Payment card result requires a compatible card cost")
        ));
    }

    #[test]
    fn migrated_token_schema_probe() {
        for chunk in EMBEDDED_TOKEN_CHUNKS {
            RON_OPTS
                .from_str::<TokenDefinition>(chunk)
                .unwrap_or_else(|error| panic!("{error}: {chunk}"));
        }
    }
}
