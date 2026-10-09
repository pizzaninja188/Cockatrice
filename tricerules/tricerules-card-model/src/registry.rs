use crate::card_def::{CardDefinition, CardFace, Layout, RawCardDefinition};
use crate::primitives::{
    AbilityCost, ActivatedCostModifier, AdditionalCost, Amount, BattlefieldAggregate,
    CardResultAction, CardResultSource, CastCostGroupDef, CastCostOptionDef,
    CastCostReceiptCondition, EffectContext, FaceChangeAction, GameCondition,
    ObjectContributionKind, ResolutionBranchRequirement, SpecialActionAffected, SpellEffectKind,
    StaticAbilityDef, TargetController, TargetKind, TargetingDef, TriggerCondition, ZoneCardFilter,
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

fn face_activated_abilities(
    face: &CardFace,
) -> impl Iterator<Item = &crate::ActivatedAbilityDef> + '_ {
    face.activated_abilities.iter().chain(
        face.class_level_bars
            .iter()
            .flat_map(|bar| &bar.activated_abilities),
    )
}

fn face_triggered_abilities(
    face: &CardFace,
) -> impl Iterator<Item = &crate::TriggeredAbilityDef> + '_ {
    face.triggered_abilities.iter().chain(
        face.class_level_bars
            .iter()
            .flat_map(|bar| &bar.triggered_abilities),
    )
}

fn face_static_abilities(
    face: &CardFace,
) -> impl Iterator<Item = &crate::IdentifiedStaticAbility> + '_ {
    face.static_abilities.iter().chain(
        face.class_level_bars
            .iter()
            .flat_map(|bar| &bar.static_abilities),
    )
}

// This traversal belongs to the new scoped grant boundary. Existing grant families retain
// their validation contracts; recipient-dependent metadata cannot be checked on the grantor.
fn visit_scoped_grant_effect(
    effect: &SpellEffectKind,
    visit: &mut impl FnMut(&SpellEffectKind) -> Result<(), String>,
) -> Result<(), String> {
    visit(effect)?;
    match effect {
        SpellEffectKind::Conditional { effect, .. }
        | SpellEffectKind::ConditionalCastCost { effect, .. } => {
            visit_scoped_grant_effect(effect, visit)?
        }
        SpellEffectKind::ChooseResolutionBranch {
            branches,
            otherwise,
            ..
        } => {
            for nested in branches
                .iter()
                .flat_map(|branch| &branch.effects)
                .chain(otherwise)
            {
                visit_scoped_grant_effect(nested, visit)?;
            }
        }
        SpellEffectKind::MayBehold { if_beheld, .. } => {
            for nested in if_beheld {
                visit_scoped_grant_effect(nested, visit)?;
            }
        }
        SpellEffectKind::ApplyPermanentModifier {
            modifier: crate::primitives::ResolvingPermanentModifier::GrantActivatedAbility(ability),
            ..
        } => {
            for nested in &ability.effect {
                visit_scoped_grant_effect(nested, visit)?;
            }
        }
        SpellEffectKind::CreateReflexiveTrigger { ability, .. } => {
            for nested in &ability.effect {
                visit_scoped_grant_effect(nested, visit)?;
            }
        }
        SpellEffectKind::GrantTriggeredAbility { ability, .. }
        | SpellEffectKind::CreateDelayedTrigger { ability, .. } => {
            for nested in ability.effect.iter().chain(
                ability
                    .modal
                    .iter()
                    .flat_map(|modal| &modal.modes)
                    .flat_map(|mode| &mode.effects),
            ) {
                visit_scoped_grant_effect(nested, visit)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_scoped_granted_activation_metadata(
    ability: &crate::ActivatedAbilityDef,
) -> Result<(), String> {
    if ability.source_zone != crate::AbilitySourceZone::Battlefield {
        return Err("scoped granted activation requires a battlefield source".into());
    }
    if ability.intrinsic_land_mana {
        return Err("intrinsic land mana cannot be an independently granted ability".into());
    }
    ability.validate_shape()?;
    validate_effect_list_metadata(&ability.effect)?;
    if ability.cost_modifiers.iter().any(|modifier| {
        matches!(
            modifier,
            ActivatedCostModifier::ConditionalSourceManaCostReduction { .. }
        )
    }) {
        return Err(
            "scoped grants do not support recipient-dependent source mana cost reduction".into(),
        );
    }
    validate_scoped_grant_targeting(ability.targeting.as_ref())?;
    let allowed = ability_cost_result_actions(&ability.costs);
    validate_scoped_grant_payment_effects(&ability.effect, &allowed)?;
    Ok(())
}

fn validate_scoped_grant_targeting(targeting: Option<&TargetingDef>) -> Result<(), String> {
    if targeting
        .iter()
        .flat_map(|targeting| &targeting.groups)
        .any(|group| group.cast_cost_expansion.is_some())
    {
        return Err("scoped grants cannot reference cast-cost target expansion".into());
    }
    Ok(())
}

fn validate_scoped_grant_condition(condition: &GameCondition) -> Result<(), String> {
    if condition.any_node_matches(|condition| matches!(condition, GameCondition::CastOrigin { .. }))
    {
        return Err("CastOrigin is available only as a face cast condition".into());
    }
    Ok(())
}

fn scoped_grant_effect_amount(effect: &SpellEffectKind) -> Option<&Amount> {
    match effect {
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
        | SpellEffectKind::TargetPlayerDraws { count: amount, .. }
        | SpellEffectKind::GainLife { amount }
        | SpellEffectKind::TargetPlayerGainsLife { amount, .. }
        | SpellEffectKind::Mill { count: amount, .. }
        | SpellEffectKind::PutCounters { count: amount, .. }
        | SpellEffectKind::PutCountersAll { count: amount, .. }
        | SpellEffectKind::PutCountersAllPlaneswalkers { count: amount, .. }
        | SpellEffectKind::Amass { count: amount, .. }
        | SpellEffectKind::CreateTokens { count: amount, .. }
        | SpellEffectKind::CreateTokenCopies { count: amount, .. }
        | SpellEffectKind::CreateAttackingTokens { count: amount, .. } => Some(amount),
        SpellEffectKind::PumpTarget {
            scale: Some(scale), ..
        } => scale.amount(),
        _ => None,
    }
}

fn validate_scoped_grant_amount(
    amount: &Amount,
    allowed: &[CardResultAction],
) -> Result<(), String> {
    if amount.card_result_filter().is_some_and(|filter| {
        filter.source == CardResultSource::Payment && !allowed.contains(&filter.action)
    }) {
        return Err("Payment card result requires a compatible card cost".into());
    }
    if let Some(conditional) = amount.cast_cost_amount() {
        validate_cast_cost_condition(&[], &conditional.condition)?;
    }
    match amount {
        Amount::Conditional { condition, .. } => validate_scoped_grant_condition(condition)?,
        Amount::DivideRoundedDown { amount, .. } => validate_scoped_grant_amount(amount, allowed)?,
        _ => {}
    }
    Ok(())
}

fn validate_scoped_grant_payment_effects(
    effects: &[SpellEffectKind],
    allowed: &[CardResultAction],
) -> Result<(), String> {
    for effect in effects {
        validate_effect_payment_results(allowed, effect)?;
        if let Some(amount) = scoped_grant_effect_amount(effect) {
            validate_scoped_grant_amount(amount, allowed)?;
        }
        match effect {
            SpellEffectKind::Conditional { effect, .. }
            | SpellEffectKind::ConditionalCastCost { effect, .. } => {
                validate_scoped_grant_payment_effects(
                    std::slice::from_ref(effect.as_ref()),
                    allowed,
                )?
            }
            SpellEffectKind::MayBehold { if_beheld, .. } => {
                validate_scoped_grant_payment_effects(if_beheld, allowed)?
            }
            SpellEffectKind::ChooseResolutionBranch {
                branches,
                otherwise,
                ..
            } => {
                for branch in branches {
                    validate_scoped_grant_payment_effects(&branch.effects, allowed)?;
                }
                validate_scoped_grant_payment_effects(otherwise, allowed)?;
            }
            SpellEffectKind::ApplyPermanentModifier {
                modifier:
                    crate::primitives::ResolvingPermanentModifier::GrantActivatedAbility(ability),
                ..
            } => {
                validate_scoped_grant_payment_effects(
                    &ability.effect,
                    &ability_cost_result_actions(&ability.costs),
                )?;
            }
            SpellEffectKind::CreateReflexiveTrigger { ability, .. } => {
                validate_scoped_grant_payment_effects(&ability.effect, &[])?
            }
            SpellEffectKind::GrantTriggeredAbility { ability, .. }
            | SpellEffectKind::CreateDelayedTrigger { ability, .. } => {
                validate_scoped_grant_payment_effects(&ability.effect, &[])?;
                for mode in ability.modal.iter().flat_map(|modal| &modal.modes) {
                    validate_scoped_grant_payment_effects(&mode.effects, &[])?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_scoped_granted_activation(ability: &crate::ActivatedAbilityDef) -> Result<(), String> {
    validate_scoped_granted_activation_metadata(ability)?;
    for effect in &ability.effect {
        visit_scoped_grant_effect(effect, &mut |effect| {
            validate_effect_cast_cost_conditions(&[], effect)?;
            match effect {
                SpellEffectKind::Conditional { condition, .. }
                | SpellEffectKind::WinGameIf { condition } => {
                    validate_scoped_grant_condition(condition)?
                }
                SpellEffectKind::SearchLibrary {
                    conditional_destination: Some(conditional),
                    ..
                } => validate_scoped_grant_condition(&conditional.condition)?,
                SpellEffectKind::ProduceMana {
                    conditional: Some(conditional),
                    ..
                } => validate_scoped_grant_condition(&conditional.condition)?,
                SpellEffectKind::ChooseResolutionBranch { branches, .. } => {
                    for branch in branches {
                        if let ResolutionBranchRequirement::GameCondition(condition) =
                            &branch.requirement
                        {
                            validate_scoped_grant_condition(condition)?;
                        }
                    }
                }
                _ => {}
            }
            if matches!(
                effect,
                SpellEffectKind::ChangeSourceFace { .. }
                    | SpellEffectKind::ExileSourceThenReturnTransformed { .. }
                    | SpellEffectKind::AttachSource { .. }
                    | SpellEffectKind::AuraAttach { .. }
            ) || effect.uses_attached_object_subject()
                || matches!(
                    effect,
                    SpellEffectKind::Sacrifice {
                        subject: crate::primitives::EffectSubject::AttachedObject
                    } | SpellEffectKind::RemoveAllAbilities {
                        subject: crate::primitives::EffectSubject::AttachedObject,
                        ..
                    } | SpellEffectKind::GrantProtection {
                        subject: crate::primitives::EffectSubject::AttachedObject,
                        ..
                    }
                )
            {
                return Err(
                    "scoped grants do not support recipient layout or attachment dependencies"
                        .into(),
                );
            }
            if matches!(
                effect,
                SpellEffectKind::SiegeDefeat | SpellEffectKind::CastMadness { .. }
            ) {
                return Err(
                    "scoped grants cannot use engine-synthesized defeat or madness context".into(),
                );
            }
            if let SpellEffectKind::ApplyPermanentModifier {
                modifier:
                    crate::primitives::ResolvingPermanentModifier::GrantActivatedAbility(nested),
                ..
            } = effect
            {
                validate_scoped_granted_activation_metadata(nested)?;
            }
            if let SpellEffectKind::CreateReflexiveTrigger { ability, .. } = effect {
                if let Some(condition) = &ability.intervening_if {
                    validate_scoped_grant_condition(condition)?;
                }
                validate_scoped_grant_targeting(ability.targeting.as_ref())?;
            }
            if let SpellEffectKind::GrantTriggeredAbility { ability, .. }
            | SpellEffectKind::CreateDelayedTrigger { ability, .. } = effect
            {
                if let Some(condition) = &ability.intervening_if {
                    validate_scoped_grant_condition(condition)?;
                }
                validate_scoped_grant_targeting(ability.targeting.as_ref())?;
                if let Some(modal) = &ability.modal {
                    if modal.all_modes_cast_cost.is_some()
                        || modal
                            .modes
                            .iter()
                            .any(|mode| mode.linked_cast_cost.is_some())
                    {
                        return Err("scoped grants cannot reference modal cast-cost links".into());
                    }
                    for mode in &modal.modes {
                        validate_scoped_grant_targeting(mode.targeting.as_ref())?;
                    }
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn validate_scoped_grant_tokens(
    face: &CardFace,
    tokens: &HashMap<String, CardDefinition>,
) -> Result<(), String> {
    for static_ability in face_static_abilities(face) {
        let StaticAbilityDef::GrantActivatedAbilityToPermanents {
            activated_abilities,
            ..
        } = &static_ability.definition
        else {
            continue;
        };
        for effect in activated_abilities
            .iter()
            .flat_map(|ability| &ability.effect)
        {
            visit_scoped_grant_effect(effect, &mut |effect| {
                for token in effect.referenced_token_ids() {
                    if !tokens.contains_key(token) {
                        return Err(format!("CreateTokens references unknown token '{token}'"));
                    }
                }
                Ok(())
            })?;
        }
    }
    Ok(())
}

fn validate_saga_face(card: &CardDefinition, face: &CardFace) -> Result<(), RegistryError> {
    let is_saga = face
        .types
        .iter()
        .any(|card_type| card_type == "Enchantment")
        && face.types.iter().any(|card_type| card_type == "Saga");
    let chapter_abilities: Vec<_> = face_triggered_abilities(face)
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
        | SpellEffectKind::TargetPlayerDraws { count: amount, .. }
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
        SpellEffectKind::ChooseResolutionBranch {
            branches,
            otherwise,
            ..
        } => {
            for branch in branches {
                if let ResolutionBranchRequirement::CastCostReceipt(condition) = &branch.requirement
                {
                    validate_cast_cost_condition(groups, condition)?;
                }
                for nested in &branch.effects {
                    validate_effect_cast_cost_conditions(groups, nested)?;
                }
            }
            for nested in otherwise {
                validate_effect_cast_cost_conditions(groups, nested)?;
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
    let amount = effect.result_consuming_amount();
    if let Some(filter) = amount.and_then(Amount::card_result_filter) {
        if filter.source == CardResultSource::Payment && !allowed.contains(&filter.action) {
            return Err("Payment card result requires a compatible card cost".into());
        }
    }
    if let SpellEffectKind::ChooseResolutionBranch {
        branches,
        otherwise,
        ..
    } = effect
    {
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
        for nested in otherwise {
            validate_effect_payment_results(allowed, nested)?;
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

fn spell_payment_result_actions(
    costs: &[AdditionalCost],
    groups: &[CastCostGroupDef],
) -> Vec<CardResultAction> {
    let mut actions = additional_cost_result_actions(costs);
    for action in groups
        .iter()
        .flat_map(|group| &group.options)
        .filter_map(|option| match option {
            CastCostOptionDef::DiscardCard { .. } => Some(CardResultAction::Discard),
            CastCostOptionDef::TapPermanents { .. } => Some(CardResultAction::Tap),
            CastCostOptionDef::SacrificePermanent { .. } => Some(CardResultAction::Sacrifice),
            CastCostOptionDef::Blight { .. }
            | CastCostOptionDef::Mana { .. }
            | CastCostOptionDef::Behold { .. }
            | CastCostOptionDef::PayLife { .. } => None,
        })
    {
        if !actions.contains(&action) {
            actions.push(action);
        }
    }
    actions
}

fn ability_cost_result_actions(costs: &[AbilityCost]) -> Vec<CardResultAction> {
    costs
        .iter()
        .filter_map(|cost| match cost {
            AbilityCost::Discard | AbilityCost::DiscardCard { .. } | AbilityCost::DiscardSelf => {
                Some(CardResultAction::Discard)
            }
            AbilityCost::ExileSelf | AbilityCost::ExileGraveyardCards { .. } => {
                Some(CardResultAction::Exile)
            }
            AbilityCost::SacrificeSelf | AbilityCost::SacrificePermanent { .. } => {
                Some(CardResultAction::Sacrifice)
            }
            AbilityCost::Tap
            | AbilityCost::PayLife { .. }
            | AbilityCost::PayCommanderColorIdentityLife
            | AbilityCost::ReturnUnblockedAttacker
            | AbilityCost::ReturnTappedCreature
            | AbilityCost::Blight { .. }
            | AbilityCost::RemoveCounters { .. }
            | AbilityCost::RemoveXStorageCountersFromSource
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
    for identified in face.static_abilities.iter().chain(
        face.class_level_bars
            .iter()
            .flat_map(|bar| &bar.static_abilities),
    ) {
        identified
            .validate_metadata()
            .map_err(|reason| RegistryError::InvalidCard {
                id: card.id.clone(),
                reason,
            })?;
        let ability = &identified.definition;
        if let StaticAbilityDef::UntapControlledPermanentsDuringOtherPlayersUntapSteps {
            permanent_types,
        } = ability
        {
            let unique: std::collections::HashSet<_> = permanent_types.iter().collect();
            if unique.len() != permanent_types.len() {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "group untap permanent types cannot contain duplicates".into(),
                });
            }
        }
        if let StaticAbilityDef::MultiplyManaFromTappedPermanents { multiplier } = ability {
            if *multiplier < 2 {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "static replacement multiplier must be at least 2".into(),
                });
            }
        }
        if let StaticAbilityDef::AttackTax {
            generic_per_attacker,
        } = ability
        {
            if *generic_per_attacker == 0 {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "AttackTax requires a nonzero generic mana amount".into(),
                });
            }
        }
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
        if let StaticAbilityDef::AddTypesToPermanents { addition, .. } = ability {
            addition
                .validate()
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
        }
        if let StaticAbilityDef::GrantKeywordToPermanents { filter, .. }
        | StaticAbilityDef::AddTypesToPermanents { filter, .. }
        | StaticAbilityDef::GrantActivatedAbilityToPermanents { filter, .. } = ability
        {
            filter
                .validate_characteristic_constraints()
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
            if !filter.all_terminal_filters_match(|leaf| {
                matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                    && leaf.controller != crate::primitives::TargetController::DefendingPlayer
                    && leaf.tapped.is_none()
                    && leaf.power.is_none()
                    && leaf.toughness.is_none()
                    && leaf.required_keywords.is_empty()
                    && leaf.excluded_keywords.is_empty()
                    && leaf.excluded_objects.iter().all(|excluded| {
                        *excluded == crate::primitives::TargetObjectExclusion::Source
                    })
            }) {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "live static permanent effects require a supported earlier-layer scope without tapped, keyword, P/T, defending-player or attached-object constraints".into(),
                });
            }
        }
        if let StaticAbilityDef::GrantActivatedAbilityToPermanents {
            activated_abilities,
            ..
        } = ability
        {
            if activated_abilities.is_empty() {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "GrantActivatedAbilityToPermanents requires at least one ability"
                        .into(),
                });
            }
            for granted in activated_abilities {
                validate_scoped_granted_activation(granted).map_err(|reason| {
                    RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    }
                })?;
            }
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
            affected,
            condition,
            unless_cost,
        } = ability
        {
            if let Some(condition) = condition {
                condition
                    .validate_live()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
            if let Some(cost) = unless_cost {
                if affected != &crate::primitives::EntersTappedAffected::Self_ {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "entry costs require an intrinsic EntersTapped ability".into(),
                    });
                }
                match cost {
                    crate::primitives::EntryCost::PayLife { amount }
                        if *amount == 0 || *amount > i32::MAX as u32 =>
                    {
                        return Err(RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason: "entry life payment requires a positive i32 amount".into(),
                        });
                    }
                    crate::primitives::EntryCost::RevealFromHand { filter } => {
                        filter
                            .validate()
                            .map_err(|reason| RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            })?;
                    }
                    _ => {}
                }
            }
        }
        if let StaticAbilityDef::EntersWithChosenBasicLandType { untapped_cost } = ability {
            let crate::primitives::EntryCost::PayLife { amount } = untapped_cost else {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "chosen basic land type entry requires life payment".into(),
                });
            };
            if *amount == 0 || *amount > i32::MAX as u32 {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "entry life payment requires a positive i32 amount".into(),
                });
            }
            if !face.types.iter().any(|value| value == "Land") {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "EntersWithChosenBasicLandType requires a Land".into(),
                });
            }
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
                        GameCondition::ControlsCreatureTiedForGreatestPower
                            | GameCondition::BattlefieldAggregate {
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
                    matches!(
                        node,
                        GameCondition::BattlefieldCreatureCount { .. }
                            | GameCondition::OpponentHasMoreThanYou {
                                metric: crate::primitives::PlayerComparisonMetric::LandCount
                                    | crate::primitives::PlayerComparisonMetric::CreatureCount,
                            }
                    )
                }) {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "conditional layer-6/7 anthems cannot depend on derived battlefield counts until CR 613.8 dependency ordering is implemented".into(),
                    });
                }
            }
        }
        if let StaticAbilityDef::AnthemKeyword { filter, .. } = ability {
            if filter.required_keyword.is_some() {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "AnthemKeyword cannot require a current keyword until CR 613.8 layer-6 dependency ordering is implemented".into(),
                });
            }
        }
        if let StaticAbilityDef::SpellGenericReduction {
            amount,
            condition,
            spell_filter,
            ..
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
            if let Some(filter) = spell_filter {
                if filter.is_unrestricted() {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "SpellGenericReduction spell_filter must constrain the spell"
                            .into(),
                    });
                }
                filter
                    .validate()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    })?;
            }
        }
        if let StaticAbilityDef::ConditionalSelfModifier {
            condition,
            remove_creature,
            set_types,
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
            if condition.any_node_matches(|node| {
                matches!(
                    node,
                    crate::primitives::GameCondition::ControlsCreatureTiedForGreatestPower
                )
            }) {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "ControlsCreatureTiedForGreatestPower cannot drive characteristic-layer conditions until CR 613.8 dependency ordering is implemented".into(),
                });
            }
            if *delta_power == 0
                && *delta_toughness == 0
                && set_types.is_none()
                && !remove_creature
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
            if let Some(set_types) = set_types {
                set_types
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
                        crate::primitives::GameCondition::ControlsCreatureTiedForGreatestPower
                            | crate::primitives::GameCondition::BattlefieldAggregate {
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
                matches!(
                    node,
                    GameCondition::BattlefieldCreatureCount { .. }
                        | GameCondition::OpponentHasMoreThanYou {
                            metric: crate::primitives::PlayerComparisonMetric::LandCount
                                | crate::primitives::PlayerComparisonMetric::CreatureCount,
                        }
                )
            }) {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "conditional self modifiers cannot depend on derived battlefield counts until CR 613.8 dependency ordering is implemented".into(),
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
        if let StaticAbilityDef::EntersAsCopy { filter, .. }
        | StaticAbilityDef::EntersAsCopyWithHasteUntilEndOfTurn { filter } = ability
        {
            filter
                .validate_characteristic_constraints()
                .map_err(|reason| RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason,
                })?;
            if !filter.all_terminal_filters_match(|leaf| {
                matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                    && leaf.controller == TargetController::Any
                    && leaf.owner == crate::primitives::TargetOwner::Any
                    && leaf.excluded_objects.is_empty()
            }) {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "entry copy requires an untargeted Creature or AnyPermanent filter"
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
            if let Some(reference) = amount.entry_cast_cost_reference() {
                let linked = face
                    .cast_cost_groups
                    .iter()
                    .find(|group| group.group_id == reference.group_id)
                    .and_then(|group| {
                        group
                            .options
                            .iter()
                            .find(|option| option.option_id() == &reference.option_id)
                    });
                if linked
                    .and_then(CastCostOptionDef::multikicker_generic_unit)
                    .is_none()
                {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "entry cast-payment count requires its linked Multikicker option"
                            .into(),
                    });
                }
            }
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
                if filter.required_keyword.is_some() {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "EntersWithCounters creature scopes cannot require a current keyword before layer 6 is applied".into(),
                    });
                }
            }
            if amount.card_result_filter().is_some() {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "card result counts are valid only in a resolving effect list".into(),
                });
            }
            amount
                .validate_entry(matches!(
                    affected,
                    crate::primitives::EntersWithCountersAffected::Self_
                ))
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
            count,
            power_per_match,
            toughness_per_match,
            set_power,
            set_toughness,
            remove_all_abilities,
            keywords,
            protections,
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
                && count.is_none()
                && set_power.is_none()
                && set_toughness.is_none()
                && !remove_all_abilities
                && add_types.is_empty()
                && set_types.is_none()
                && set_name.is_none()
                && set_colors.is_none()
                && keywords.is_empty()
                && protections.is_empty()
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
            if let Some(expression) = count {
                expression.validate_static_count().map_err(|reason| {
                    RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    }
                })?;
                if *power_per_match == 0 && *toughness_per_match == 0 {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "AttachedModifier count scaling must modify power or toughness"
                            .into(),
                    });
                }
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
                if condition.any_node_matches(|node| {
                    matches!(
                        node,
                        crate::primitives::GameCondition::ControlsCreatureTiedForGreatestPower
                    )
                }) {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "ControlsCreatureTiedForGreatestPower cannot drive characteristic-layer conditions until CR 613.8 dependency ordering is implemented".into(),
                    });
                }
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
                    || count.is_some()
                    || set_power.is_some()
                    || *remove_all_abilities
                    || !keywords.is_empty()
                    || !protections.is_empty())
                    && condition.any_node_matches(|node| {
                        matches!(
                            node,
                            crate::primitives::GameCondition::ControlsCreatureTiedForGreatestPower
                                | crate::primitives::GameCondition::BattlefieldAggregate {
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
        if matches!(
            ability,
            StaticAbilityDef::ProhibitActivatedAbilitiesOfAttachedPermanent
        ) && !face.is_aura
        {
            return Err(RegistryError::InvalidCard {
                id: card.id.clone(),
                reason: "ProhibitActivatedAbilitiesOfAttachedPermanent requires an Aura source"
                    .into(),
            });
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
        SpellEffectKind::SetClassLevel { .. } => {
            Err("SetClassLevel may only be the sole effect of a Class level-up ability".into())
        }
        SpellEffectKind::CreateReflexiveTrigger { ability, .. } => {
            ability.validate_shape()?;
            validate_effect_list_metadata(&ability.effect)
        }
        SpellEffectKind::GrantTriggeredAbility { ability, .. }
        | SpellEffectKind::CreateDelayedTrigger { ability, .. } => {
            ability.validate_shape()?;
            validate_effect_list_metadata(&ability.effect)
        }
        SpellEffectKind::ApplyPermanentModifier {
            modifier: crate::primitives::ResolvingPermanentModifier::GrantActivatedAbility(ability),
            ..
        } => {
            ability.validate_shape()?;
            validate_effect_list_metadata(&ability.effect)
        }
        SpellEffectKind::ChooseResolutionBranch {
            branches,
            otherwise,
            ..
        } => {
            for branch in branches {
                validate_effect_list_metadata(&branch.effects)?;
            }
            validate_effect_list_metadata(otherwise)
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

fn collect_linked_exile_uses(effect: &SpellEffectKind, uses: &mut HashMap<String, (u32, u32)>) {
    match effect {
        SpellEffectKind::MoveGraveyardCards {
            linked_exile_id: Some(link_id),
            ..
        } => uses.entry(link_id.as_str().to_owned()).or_default().0 += 1,
        SpellEffectKind::ReturnLinkedExiledCards {
            linked_exile_id, ..
        } => {
            uses.entry(linked_exile_id.as_str().to_owned())
                .or_default()
                .1 += 1
        }
        SpellEffectKind::Conditional { effect, .. }
        | SpellEffectKind::ConditionalCastCost { effect, .. } => {
            collect_linked_exile_uses(effect, uses)
        }
        SpellEffectKind::ChooseResolutionBranch {
            branches,
            otherwise,
            ..
        } => {
            for effect in branches.iter().flat_map(|branch| &branch.effects) {
                collect_linked_exile_uses(effect, uses);
            }
            for effect in otherwise {
                collect_linked_exile_uses(effect, uses);
            }
        }
        SpellEffectKind::CreateReflexiveTrigger { ability, .. } => {
            for effect in &ability.effect {
                collect_linked_exile_uses(effect, uses);
            }
        }
        SpellEffectKind::GrantTriggeredAbility { ability, .. }
        | SpellEffectKind::CreateDelayedTrigger { ability, .. } => {
            for effect in &ability.effect {
                collect_linked_exile_uses(effect, uses);
            }
        }
        SpellEffectKind::ApplyPermanentModifier {
            modifier: crate::primitives::ResolvingPermanentModifier::GrantActivatedAbility(ability),
            ..
        } => {
            for effect in &ability.effect {
                collect_linked_exile_uses(effect, uses);
            }
        }
        _ => {}
    }
}

fn validate_chosen_opponent_links(face: &CardFace) -> Result<(), String> {
    let producers = face
        .static_abilities
        .iter()
        .filter_map(|ability| match &ability.definition {
            StaticAbilityDef::AsEntersChooseOpponent { link_id } => Some(link_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    let consumers = face
        .triggered_abilities
        .iter()
        .filter_map(|ability| match &ability.trigger {
            TriggerCondition::AtBeginningOfChosenPlayerUpkeep { link_id } => Some(link_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    if producers.is_empty() && consumers.is_empty() {
        return Ok(());
    }
    if producers.len() != 1 || consumers.len() != 1 || producers[0] != consumers[0] {
        return Err(
            "chosen-opponent link requires one matching printed producer and consumer".into(),
        );
    }
    producers[0].validate()
}

fn collect_chosen_creature_type_links(
    filter: &crate::primitives::PermanentEventFilter,
    links: &mut Vec<crate::AbilityLinkId>,
) {
    if let Some(link_id) = &filter.source_chosen_creature_type {
        links.push(link_id.clone());
    }
    for branch in filter.any_of.iter().flatten() {
        collect_chosen_creature_type_links(branch, links);
    }
}

fn validate_chosen_creature_type_links(face: &CardFace) -> Result<(), String> {
    let producers = face
        .static_abilities
        .iter()
        .filter_map(|ability| match &ability.definition {
            StaticAbilityDef::AsEntersChooseCreatureType { link_id } => Some(link_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut consumers = Vec::new();
    for ability in &face.triggered_abilities {
        if let TriggerCondition::WheneverPermanentEntersBattlefield { filter, .. } =
            &ability.trigger
        {
            collect_chosen_creature_type_links(filter, &mut consumers);
        }
    }
    if producers.is_empty() && consumers.is_empty() {
        return Ok(());
    }
    if producers.len() != 1 || consumers.len() != 1 || producers[0] != &consumers[0] {
        return Err(
            "chosen-creature-type link requires one matching printed producer and observer".into(),
        );
    }
    producers[0].validate()
}

fn validate_granted_chosen_opponent(ability: &crate::TriggeredAbilityDef) -> Result<(), String> {
    if matches!(
        ability.trigger,
        TriggerCondition::AtBeginningOfChosenPlayerUpkeep { .. }
    ) {
        return Err("chosen-opponent upkeep links must be a printed face-local pair".into());
    }
    Ok(())
}

fn validate_linked_exile_pairs(face: &CardFace) -> Result<(), String> {
    let mut uses = HashMap::new();
    let collect = |effects: &[SpellEffectKind], uses: &mut HashMap<String, (u32, u32)>| {
        for effect in effects {
            collect_linked_exile_uses(effect, uses);
        }
    };
    collect(&face.spell_effect, &mut uses);
    for mode in face.modal_spell.iter().flat_map(|modal| &modal.modes) {
        collect(&mode.effects, &mut uses);
    }
    for ability in face_activated_abilities(face) {
        collect(&ability.effect, &mut uses);
    }
    for ability in face_triggered_abilities(face) {
        collect(&ability.effect, &mut uses);
    }
    for ability in face_static_abilities(face) {
        match &ability.definition {
            StaticAbilityDef::AttachedModifier {
                activated_abilities,
                triggered_abilities,
                ..
            }
            | StaticAbilityDef::ConditionalSelfModifier {
                activated_abilities,
                triggered_abilities,
                ..
            } => {
                for ability in activated_abilities {
                    collect(&ability.effect, &mut uses);
                }
                for ability in triggered_abilities {
                    collect(&ability.effect, &mut uses);
                }
            }
            StaticAbilityDef::GrantActivatedAbilityToPermanents {
                activated_abilities,
                ..
            } => {
                for ability in activated_abilities {
                    // Scoped traversal also includes MayBehold and nested triggered modes.
                    // Collect direct leaves exactly once instead of recursively collecting twice.
                    for effect in &ability.effect {
                        visit_scoped_grant_effect(effect, &mut |effect| {
                            match effect {
                                SpellEffectKind::MoveGraveyardCards {
                                    linked_exile_id: Some(link_id),
                                    ..
                                } => uses.entry(link_id.as_str().to_owned()).or_default().0 += 1,
                                SpellEffectKind::ReturnLinkedExiledCards {
                                    linked_exile_id,
                                    ..
                                } => {
                                    uses.entry(linked_exile_id.as_str().to_owned())
                                        .or_default()
                                        .1 += 1
                                }
                                _ => {}
                            }
                            Ok(())
                        })?;
                    }
                }
            }
            StaticAbilityDef::GrantTriggeredAbilityToPermanents {
                triggered_abilities,
                ..
            } => {
                for ability in triggered_abilities {
                    collect(&ability.effect, &mut uses);
                }
            }
            _ => {}
        }
    }
    for (link_id, (producers, consumers)) in uses {
        if producers != 1 || consumers != 1 {
            return Err(format!(
                "linked exile id '{link_id}' requires exactly one producer and one consumer"
            ));
        }
    }
    Ok(())
}

fn effect_returns_source_transformed(effect: &SpellEffectKind) -> bool {
    match effect {
        SpellEffectKind::ExileSourceThenReturnTransformed { .. } => true,
        SpellEffectKind::Conditional { effect, .. }
        | SpellEffectKind::ConditionalCastCost { effect, .. } => {
            effect_returns_source_transformed(effect)
        }
        SpellEffectKind::ChooseResolutionBranch {
            branches,
            otherwise,
            ..
        } => {
            branches
                .iter()
                .flat_map(|branch| &branch.effects)
                .any(effect_returns_source_transformed)
                || otherwise.iter().any(effect_returns_source_transformed)
        }
        SpellEffectKind::CreateReflexiveTrigger { ability, .. } => {
            ability.effect.iter().any(effect_returns_source_transformed)
        }
        SpellEffectKind::GrantTriggeredAbility { ability, .. }
        | SpellEffectKind::CreateDelayedTrigger { ability, .. } => {
            ability.effect.iter().any(effect_returns_source_transformed)
        }
        _ => false,
    }
}

fn face_returns_source_transformed(face: &CardFace) -> bool {
    face.spell_effect
        .iter()
        .chain(
            face.modal_spell
                .iter()
                .flat_map(|modal| &modal.modes)
                .flat_map(|mode| &mode.effects),
        )
        .chain(face_activated_abilities(face).flat_map(|ability| &ability.effect))
        .chain(face_triggered_abilities(face).flat_map(|ability| &ability.effect))
        .any(effect_returns_source_transformed)
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

fn validate_retained_exile_cohorts(
    effects: &[SpellEffectKind],
    direct_spell: bool,
) -> Result<(), String> {
    let mut producers = HashSet::new();
    let mut consumers = HashSet::new();
    for effect in effects {
        let binding = match effect {
            SpellEffectKind::ExileGraveyards {
                capture_exile_cohort: Some(id),
                ..
            } => Some((id, true)),
            SpellEffectKind::ReturnExiledCohortToOwnersBattlefield { cohort_id } => {
                Some((cohort_id, false))
            }
            _ => None,
        };
        if let Some((id, producer)) = binding {
            if !direct_spell {
                return Err(
                    "retained exile cohorts require a direct nonmodal spell effect list".into(),
                );
            }
            if producer {
                if !producers.insert(id.as_str()) {
                    return Err(format!("duplicate retained exile cohort producer '{id}'"));
                }
            } else if !producers.contains(id.as_str()) || !consumers.insert(id.as_str()) {
                return Err(format!(
                    "retained exile cohort '{id}' requires one earlier producer and one consumer"
                ));
            }
        }
        match effect {
            SpellEffectKind::Conditional { effect, .. }
            | SpellEffectKind::ConditionalCastCost { effect, .. } => {
                validate_retained_exile_cohorts(std::slice::from_ref(effect), false)?;
            }
            SpellEffectKind::MayBehold { if_beheld, .. } => {
                validate_retained_exile_cohorts(if_beheld, false)?
            }
            SpellEffectKind::ChooseResolutionBranch {
                branches,
                otherwise,
                ..
            } => {
                for branch in branches {
                    validate_retained_exile_cohorts(&branch.effects, false)?;
                }
                validate_retained_exile_cohorts(otherwise, false)?;
            }
            _ => {}
        }
    }
    if producers != consumers {
        return Err("every retained exile cohort requires one later consumer".into());
    }
    Ok(())
}

fn validate_face_identity(face: &CardFace) -> Result<(), String> {
    validate_retained_exile_cohorts(&face.spell_effect, face.modal_spell.is_none())?;
    if let Some(modal) = &face.modal_spell {
        for mode in &modal.modes {
            validate_retained_exile_cohorts(&mode.effects, false)?;
        }
    }
    face.face_id.validate()?;
    if face
        .activated_abilities
        .iter()
        .filter(|ability| ability.intrinsic_land_mana)
        .count()
        > 1
    {
        return Err("a face may have only one intrinsic land mana bundle".into());
    }
    let mut siblings = HashSet::new();
    for ability in &face.activated_abilities {
        insert_ability_id(&mut siblings, &ability.ability_id)?;
        ability.validate_shape()?;
        if ability.intrinsic_land_mana {
            let expected = crate::BasicLandType::ALL
                .into_iter()
                .filter(|land_type| face.types.iter().any(|value| value == land_type.as_str()))
                .map(crate::BasicLandType::mana)
                .collect::<Vec<_>>();
            let matching_output = matches!(ability.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, commander_color_identity: false, restriction: None, conditional: None }]
                if options.len() == expected.len() && expected.iter().all(|mana| options.contains(mana)));
            if !face.types.iter().any(|value| value == "Land")
                || expected.is_empty()
                || !matching_output
                || ability.source_zone != crate::AbilitySourceZone::Battlefield
                || ability.costs.as_slice() != [AbilityCost::Tap]
                || !ability.cost_modifiers.is_empty()
                || ability.targeting.is_some()
                || ability.timing != crate::ActivationTiming::Normal
                || !ability.conditions.is_empty()
                || ability.activation_limit.is_some()
            {
                return Err("intrinsic land mana must be the unrestricted tap-only bundle of the face's basic land subtypes".into());
            }
        }
        validate_source_mana_cost_reduction(face, ability)?;
        validate_effect_list_metadata(&ability.effect)?;
    }
    for ability in &face.triggered_abilities {
        insert_ability_id(&mut siblings, &ability.ability_id)?;
        ability.validate_shape()?;
        if matches!(
            ability.trigger,
            TriggerCondition::WhenThisClassBecomesLevel { .. }
        ) {
            return Err(
                "Class-level transition triggers must appear in their matching level bar".into(),
            );
        }
        validate_effect_list_metadata(&ability.effect)?;
    }
    validate_class_level_bars(face, &mut siblings)?;
    for ability in face.static_abilities.iter().chain(
        face.class_level_bars
            .iter()
            .flat_map(|bar| &bar.static_abilities),
    ) {
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
                    if nested_ability.intrinsic_land_mana {
                        return Err(
                            "intrinsic land mana cannot be an independently granted ability".into(),
                        );
                    }
                    nested_ability.validate_shape()?;
                    validate_effect_list_metadata(&nested_ability.effect)?;
                }
                for nested_ability in triggered_abilities {
                    insert_ability_id(&mut nested, &nested_ability.ability_id)?;
                    validate_granted_chosen_opponent(nested_ability)?;
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
                    if nested_ability.intrinsic_land_mana {
                        return Err(
                            "intrinsic land mana cannot be an independently granted ability".into(),
                        );
                    }
                    nested_ability.validate_shape()?;
                    validate_effect_list_metadata(&nested_ability.effect)?;
                }
                for nested_ability in triggered_abilities {
                    insert_ability_id(&mut nested, &nested_ability.ability_id)?;
                    validate_granted_chosen_opponent(nested_ability)?;
                    nested_ability.validate_shape()?;
                    validate_effect_list_metadata(&nested_ability.effect)?;
                }
            }
            StaticAbilityDef::GrantActivatedAbilityToPermanents {
                activated_abilities,
                ..
            } => {
                for nested_ability in activated_abilities {
                    insert_ability_id(&mut nested, &nested_ability.ability_id)?;
                    validate_scoped_granted_activation(nested_ability)?;
                }
            }
            StaticAbilityDef::GrantTriggeredAbilityToPermanents {
                triggered_abilities,
                ..
            } => {
                for nested_ability in triggered_abilities {
                    insert_ability_id(&mut nested, &nested_ability.ability_id)?;
                    validate_granted_chosen_opponent(nested_ability)?;
                    nested_ability.validate_shape()?;
                    validate_effect_list_metadata(&nested_ability.effect)?;
                }
            }
            _ => {}
        }
    }
    let mut defines_colors = false;
    for ability in &face.characteristic_defining_abilities {
        insert_ability_id(&mut siblings, &ability.ability_id)?;
        ability.validate_metadata()?;
        ability.definition.validate()?;
        if matches!(
            &ability.definition,
            crate::CharacteristicDefiningAbility::Devoid
                | crate::CharacteristicDefiningAbility::DefinesColors { .. }
        ) {
            if defines_colors {
                return Err("a face may have only one color-defining CDA".into());
            }
            defines_colors = true;
        }
    }
    validate_linked_exile_pairs(face)?;
    validate_chosen_opponent_links(face)?;
    validate_chosen_creature_type_links(face)?;
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

fn validate_class_level_bars(
    face: &CardFace,
    siblings: &mut HashSet<String>,
) -> Result<(), String> {
    let is_class = face.types.iter().any(|kind| kind == "Class");
    if is_class == face.class_level_bars.is_empty() {
        return Err(if is_class {
            "a Class face must define its level bars"
        } else {
            "class level bars require a Class face"
        }
        .into());
    }
    if is_class && !face.types.iter().any(|kind| kind == "Enchantment") {
        return Err("Class faces must be enchantments".into());
    }
    if is_class
        && face
            .class_level_bars
            .first()
            .is_some_and(|bar| bar.level != 2)
    {
        return Err("Class level bars must begin at level 2".into());
    }
    if is_class && face.class_level_bars.len() != 2 {
        return Err("a Class face requires level bars 2 and 3".into());
    }

    let mut expected_level = 2u32;
    for bar in &face.class_level_bars {
        if bar.level != expected_level {
            return Err(if expected_level == 2 {
                "Class level bars must begin at level 2"
            } else {
                "Class level bars must be consecutive"
            }
            .into());
        }
        let level_ability = &bar.level_ability;
        insert_ability_id(siblings, &level_ability.ability_id)?;
        level_ability.validate_shape()?;
        if level_ability.source_zone != crate::AbilitySourceZone::Battlefield
            || level_ability.intrinsic_land_mana
            || level_ability.timing != crate::ActivationTiming::SorcerySpeed
            || level_ability.targeting.is_some()
            || !level_ability.conditions.is_empty()
            || !matches!(
                level_ability.effect.as_slice(),
                [SpellEffectKind::SetClassLevel { level }] if *level == bar.level
            )
        {
            return Err(format!(
                "Class level {} ability must be an untargeted sorcery-speed battlefield ability whose sole effect sets that level",
                bar.level
            ));
        }

        for ability in &bar.activated_abilities {
            insert_ability_id(siblings, &ability.ability_id)?;
            ability.validate_shape()?;
            if ability.source_zone != crate::AbilitySourceZone::Battlefield
                || ability.intrinsic_land_mana
            {
                return Err(
                    "Class section activated abilities require a battlefield source".into(),
                );
            }
            validate_source_mana_cost_reduction(face, ability)?;
            validate_effect_list_metadata(&ability.effect)?;
        }
        for ability in &bar.triggered_abilities {
            insert_ability_id(siblings, &ability.ability_id)?;
            ability.validate_shape()?;
            if matches!(
                &ability.trigger,
                TriggerCondition::WhenThisClassBecomesLevel { level }
                    if *level != bar.level
            ) {
                return Err(format!(
                    "Class-level transition trigger in the level {} bar must name level {}",
                    bar.level, bar.level
                ));
            }
            validate_effect_list_metadata(&ability.effect)?;
            if let Some(modal) = &ability.modal {
                for mode in &modal.modes {
                    validate_effect_list_metadata(&mode.effects)?;
                }
            }
        }
        if bar.static_abilities.iter().any(|ability| {
            matches!(
                &ability.definition,
                StaticAbilityDef::EntersPrepared
                    | StaticAbilityDef::EntersAsCopy { .. }
                    | StaticAbilityDef::EntersAsCopyWithHasteUntilEndOfTurn { .. }
                    | StaticAbilityDef::EntersWithChosenBasicLandType { .. }
                    | StaticAbilityDef::AsEntersChooseOpponent { .. }
                    | StaticAbilityDef::AsEntersChooseCreatureType { .. }
                    | StaticAbilityDef::EntersTapped { .. }
                    | StaticAbilityDef::EntersWithCounters { .. }
                    | StaticAbilityDef::SpellCannotBeCountered
                    | StaticAbilityDef::Madness { .. }
                    | StaticAbilityDef::GraveyardAnthemKeyword { .. }
            )
        }) {
            return Err(
                "Class section static ability must function while the Class level is active on the battlefield"
                    .into(),
            );
        }

        expected_level = bar
            .level
            .checked_add(1)
            .ok_or_else(|| "Class level exceeds the supported level range".to_string())?;
    }
    Ok(())
}

impl CardRegistry {
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
            validate_scoped_grant_tokens(face, &reg.tokens).map_err(|reason| {
                RegistryError::InvalidCard {
                    id: id.clone(),
                    reason,
                }
            })?;
            validate_saga_face(token, face)?;
            let can_reference_attached_object = face_can_reference_attached_object(face);
            let can_reference_attached_player = face_can_reference_attached_player(face);
            for ability in face.activated_abilities.iter().chain(
                face.class_level_bars
                    .iter()
                    .flat_map(|bar| &bar.activated_abilities),
            ) {
                ability
                    .validate_shape()
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: id.clone(),
                        reason,
                    })?;
                for effect in &ability.effect {
                    for token in effect.referenced_token_ids() {
                        if !reg.tokens.contains_key(token) {
                            return Err(RegistryError::InvalidCard {
                                id: id.clone(),
                                reason: format!("CreateTokens references unknown token '{token}'"),
                            });
                        }
                    }
                }
            }
            for ability in face.triggered_abilities.iter().chain(
                face.class_level_bars
                    .iter()
                    .flat_map(|bar| &bar.triggered_abilities),
            ) {
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
                    if condition.requires_observed_object_context()
                        && !ability.trigger.observes_permanent_entry()
                    {
                        return Err(RegistryError::InvalidCard {
                            id: id.clone(),
                            reason: "event-observed condition requires a permanent-entry observer"
                                .into(),
                        });
                    }
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
                    for token in effect.referenced_token_ids() {
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
            if matches!(card.layout, Layout::Adventure | Layout::Preparation) {
                let valid_roles = card.faces.len() == 2
                    && card.faces[0].is_permanent()
                    && (card.faces[1].is_instant || card.faces[1].is_sorcery)
                    && !card.faces[1].is_permanent();
                if !valid_roles {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: format!("{:?} requires exactly two faces: permanent face 0 and instant/sorcery face 1", card.layout),
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
            if card.faces_iter().any(face_returns_source_transformed)
                && !(card.layout == Layout::Transform
                    && card.faces.len() == 2
                    && card.faces[1].is_permanent())
            {
                return Err(RegistryError::InvalidCard {
                    id: card.id.clone(),
                    reason: "transformed source return requires a two-face Transform card with a permanent back face"
                        .into(),
                });
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
                    condition.validate_cast_condition().map_err(|reason| {
                        RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        }
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
                let payment_actions =
                    spell_payment_result_actions(&face.additional_costs, &face.cast_cost_groups);
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
                // CR 113.6g is the stack-active exception to CR 604.2. Every other static ability
                // here requires a permanent source on the battlefield; an instant/sorcery anthem,
                // for example, belongs in `spell_effect` as a one-shot `PumpAll`.
                if (face.is_instant || face.is_sorcery)
                    && face.static_abilities.iter().any(|ability| {
                        !matches!(
                            ability.definition,
                            StaticAbilityDef::SpellCannotBeCountered
                                | StaticAbilityDef::Madness { .. }
                        )
                    })
                {
                    return Err(RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason: "only stack-active or madness static abilities are valid on instant/sorcery"
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
                        face.class_level_bars
                            .iter()
                            .flat_map(|bar| &bar.activated_abilities)
                            .flat_map(|ability| &ability.effect),
                    )
                    .chain(
                        face.class_level_bars
                            .iter()
                            .flat_map(|bar| &bar.triggered_abilities)
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
                    .chain(
                        face.class_level_bars
                            .iter()
                            .flat_map(|bar| &bar.activated_abilities)
                            .flat_map(|ability| &ability.effect),
                    )
                    .chain(
                        face.class_level_bars
                            .iter()
                            .flat_map(|bar| &bar.triggered_abilities)
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
                    .chain(
                        face.class_level_bars
                            .iter()
                            .flat_map(|bar| &bar.activated_abilities)
                            .flat_map(|ability| &ability.effect),
                    )
                    .chain(
                        face.class_level_bars
                            .iter()
                            .flat_map(|bar| &bar.triggered_abilities)
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
                validate_scoped_grant_tokens(face, &reg.tokens).map_err(|reason| {
                    RegistryError::InvalidCard {
                        id: card.id.clone(),
                        reason,
                    }
                })?;
                validate_saga_face(&card, face)?;
                for ability in face.triggered_abilities.iter().chain(
                    face.class_level_bars
                        .iter()
                        .flat_map(|bar| &bar.triggered_abilities),
                ) {
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
                        if condition.requires_observed_object_context()
                            && !ability.trigger.observes_permanent_entry()
                        {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason:
                                    "event-observed condition requires a permanent-entry observer"
                                        .into(),
                            });
                        }
                    }
                }
                // An ability's effect list gets the same two checks a spell's does: each effect
                // against its context, then the list as a whole (CR 608.2 — the effects resolve
                // together, so a cross-effect requirement like `LoseLife(TargetManaValue)` must
                // find its object-targeting sibling inside this one ability).
                for ability in face_activated_abilities(face) {
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
                for ability in face_triggered_abilities(face) {
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
                    .chain(face.class_level_bars.iter().flat_map(|bar| {
                        std::iter::once(&bar.level_ability.effect)
                            .chain(bar.activated_abilities.iter().map(|a| &a.effect))
                            .chain(bar.triggered_abilities.iter().map(|t| &t.effect))
                    }))
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
                // Activated groups were checked by validate_shape, including CR 602.3 chooser
                // authority. Triggers retain the ordinary controller-owned target contract.
                for (effects, targeting) in face
                    .triggered_abilities
                    .iter()
                    .map(|ability| (&ability.effect, ability.targeting.as_ref()))
                    .chain(face.class_level_bars.iter().flat_map(|bar| {
                        bar.triggered_abilities
                            .iter()
                            .map(|ability| (&ability.effect, ability.targeting.as_ref()))
                    }))
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
                    .chain(
                        face.class_level_bars
                            .iter()
                            .flat_map(|bar| &bar.activated_abilities)
                            .flat_map(|ability| &ability.effect),
                    )
                    .chain(face.triggered_abilities.iter().flat_map(|t| &t.effect))
                    .chain(
                        face.class_level_bars
                            .iter()
                            .flat_map(|bar| &bar.triggered_abilities)
                            .flat_map(|ability| &ability.effect),
                    );
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
                    for token in effect.referenced_token_ids() {
                        if !reg.tokens.contains_key(token) {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: format!("CreateTokens references unknown token '{token}'"),
                            });
                        }
                    }
                }
                for modal in face
                    .modal_spell
                    .iter()
                    .chain(
                        face.triggered_abilities
                            .iter()
                            .filter_map(|ability| ability.modal.as_ref()),
                    )
                    .chain(
                        face.class_level_bars
                            .iter()
                            .flat_map(|bar| &bar.triggered_abilities)
                            .filter_map(|ability| ability.modal.as_ref()),
                    )
                {
                    for effect in modal.modes.iter().flat_map(|mode| &mode.effects) {
                        for token in effect.referenced_token_ids() {
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
            // Only physical-card aliases belong in deck admission. Inset spell names may
            // collide with separately printed cards and must not claim those cards' support.
            for name in card.deck_input_names() {
                if reg
                    .by_name
                    .insert(normalize_name(name), id.clone())
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

    /// Attach the embedded corpus's external-presentation compatibility metadata.
    /// Metadata is descriptive only; it does not participate in rules resolution.
    pub fn with_presentation_faces(
        mut self,
        faces: impl IntoIterator<Item = ((String, String), PresentationFaceMetadata)>,
    ) -> Self {
        self.presentation_faces.extend(faces);
        self
    }
}
