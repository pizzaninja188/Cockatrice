use crate::card_def::{CardDefinition, CardFace, Layout, RawCardDefinition};
use crate::primitives::{
    AbilityCost, AdditionalCost, Amount, BattlefieldAggregate, CardResultAction, CardResultSource,
    CastCostGroupDef, CastCostReceiptCondition, EffectContext, FaceChangeAction, GameCondition,
    InterveningIf, ResolutionBranchRequirement, SpecialActionAffected, SpellEffectKind,
    StaticAbilityDef, TargetController, TargetKind, TargetingDef, TriggerCondition,
};
use crate::token_def::TokenDefinition;
use once_cell::sync::Lazy;
use ron::extensions::Extensions;
use ron::Options;
use std::collections::HashMap;
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

fn face_can_reference_attached_player(face: &CardFace) -> bool {
    face.is_aura
        && face.spell_effect.iter().any(
            |effect| matches!(effect, SpellEffectKind::AuraAttach { target } if target.is_player()),
        )
}

fn validate_cast_cost_condition(
    groups: &[CastCostGroupDef],
    condition: CastCostReceiptCondition,
) -> Result<(), String> {
    let group = groups
        .get(condition.group_index as usize)
        .ok_or_else(|| "cast-cost condition references an unknown group".to_string())?;
    if group.options.get(condition.option_index as usize).is_none() {
        return Err("cast-cost condition references an unknown option".into());
    }
    Ok(())
}

fn validate_effect_cast_cost_conditions(
    groups: &[CastCostGroupDef],
    effect: &SpellEffectKind,
) -> Result<(), String> {
    match effect {
        SpellEffectKind::CounterTargetSpell {
            unless_controller_pays_by_cast_cost: Some(conditional),
            ..
        }
        | SpellEffectKind::SearchLibrary {
            count_by_cast_cost: Some(conditional),
            ..
        } => validate_cast_cost_condition(groups, conditional.condition),
        SpellEffectKind::ChooseResolutionBranch { branches, .. } => {
            for branch in branches {
                if let ResolutionBranchRequirement::CastCostReceipt(condition) = branch.requirement
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
        | SpellEffectKind::DamagePlayer { amount, .. }
        | SpellEffectKind::Draw { count: amount, .. }
        | SpellEffectKind::GainLife { amount }
        | SpellEffectKind::Mill { count: amount, .. }
        | SpellEffectKind::CreateTokens { count: amount, .. } => Some(amount),
        SpellEffectKind::PumpTarget {
            scale: Some(scale), ..
        } => Some(&scale.amount),
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
        .map(|cost| match cost {
            AdditionalCost::DiscardCard => CardResultAction::Discard,
            AdditionalCost::SacrificePermanent { .. } => CardResultAction::Sacrifice,
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
            AbilityCost::Tap | AbilityCost::Mana(_) => None,
        })
        .collect()
}

impl CardRegistry {
    pub fn from_embedded() -> Result<Self, RegistryError> {
        Self::from_chunks_and_tokens(EMBEDDED_RON_CHUNKS, EMBEDDED_TOKEN_CHUNKS)
    }

    #[cfg(test)]
    fn from_chunks(chunks: &[&str]) -> Result<Self, RegistryError> {
        Self::from_chunks_and_tokens(chunks, &[])
    }

    fn from_chunks_and_tokens(
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
                    if let SpellEffectKind::CreateTokens { token, .. } = effect {
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
                    .map_err(|reason| RegistryError::InvalidCard {
                        id: id.clone(),
                        reason,
                    })?;
                match &ability.trigger {
                    TriggerCondition::WheneverAttachedObjectAttacks
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
                if ability.text.trim().is_empty() {
                    return Err(RegistryError::InvalidCard {
                        id: id.clone(),
                        reason: "token triggered ability text must not be empty".into(),
                    });
                }
                if ability.effect.is_empty() {
                    return Err(RegistryError::InvalidCard {
                        id: id.clone(),
                        reason: "token triggered ability must contain at least one effect".into(),
                    });
                }
                if let Some(InterveningIf::GameCondition(condition)) =
                    ability.intervening_if.as_ref()
                {
                    condition
                        .validate()
                        .map_err(|reason| RegistryError::InvalidCard {
                            id: id.clone(),
                            reason,
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
                    if let SpellEffectKind::CreateTokens { token, .. } = effect {
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
            for face in card.faces_iter() {
                for modifier in &face.cost_modifiers {
                    modifier
                        .validate()
                        .map_err(|reason| RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        })?;
                }
                if let Some(condition) = face.instant_speed_cast_cost {
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
                let spell_payment_actions = additional_cost_result_actions(&face.additional_costs);
                for effect in &face.spell_effect {
                    validate_effect_payment_results(&spell_payment_actions, effect).map_err(
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
                            validate_effect_payment_results(&spell_payment_actions, effect)
                                .map_err(|reason| RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason,
                                })?;
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
                let attachment_source = face.is_aura || face.types.iter().any(|t| t == "Equipment");
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
                for ability in &face.static_abilities {
                    if let StaticAbilityDef::EntersTapped {
                        condition: Some(condition),
                        ..
                    } = ability
                    {
                        condition
                            .validate()
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
                        if let crate::primitives::TargetingCostProtected::Creatures(filter) =
                            protected
                        {
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
                                .validate()
                                .map_err(|reason| RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason,
                                })?;
                            if matches!(
                                condition,
                                GameCondition::BattlefieldAggregate {
                                    aggregate: BattlefieldAggregate::TotalPower
                                        | BattlefieldAggregate::MaximumPower,
                                    ..
                                }
                            ) {
                                return Err(RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason: "conditional layer-6/7 anthems cannot depend on battlefield power until CR 613.8 dependency ordering is implemented".into(),
                                });
                            }
                            if matches!(condition, GameCondition::BattlefieldCreatureCount { .. }) {
                                return Err(RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason: "conditional layer-6/7 anthems cannot depend on derived creature counts until CR 613.8 dependency ordering is implemented".into(),
                                });
                            }
                        }
                    }
                    if let StaticAbilityDef::ConditionalSelfModifier {
                        condition,
                        delta_power,
                        delta_toughness,
                        keywords,
                        can_attack_as_though_without_defender,
                    } = ability
                    {
                        condition
                            .validate()
                            .map_err(|reason| RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            })?;
                        if *delta_power == 0
                            && *delta_toughness == 0
                            && keywords.is_empty()
                            && !can_attack_as_though_without_defender
                        {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: "ConditionalSelfModifier must modify at least one value"
                                    .into(),
                            });
                        }
                        if (*delta_power != 0 || *delta_toughness != 0 || !keywords.is_empty())
                            && matches!(
                                condition,
                                crate::primitives::GameCondition::BattlefieldAggregate {
                                    aggregate: BattlefieldAggregate::TotalPower
                                        | BattlefieldAggregate::MaximumPower,
                                    ..
                                }
                            )
                        {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: "conditional layer-6/7 modifiers cannot depend on battlefield power until CR 613.8 dependency ordering is implemented".into(),
                            });
                        }
                        if matches!(condition, GameCondition::BattlefieldCreatureCount { .. }) {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: "conditional self modifiers cannot depend on derived creature counts until CR 613.8 dependency ordering is implemented".into(),
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
                                && !leaf.exclude_source
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
                            validate_cast_cost_condition(&face.cast_cost_groups, *condition)
                                .map_err(|reason| RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason,
                                })?;
                        }
                        counter
                            .validate()
                            .map_err(|reason| RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            })?;
                        if let crate::primitives::EntersWithCountersAffected::Creatures(filter) =
                            affected
                        {
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
                                reason:
                                    "card result counts are valid only in a resolving effect list"
                                        .into(),
                            });
                        }
                        amount
                            .validate()
                            .map_err(|reason| RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            })?;
                    }
                    if let StaticAbilityDef::PreventDamage {
                        additional_effect:
                            Some(crate::primitives::DamagePreventionAdditionalEffect::PutCounters {
                                counter,
                                ..
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
                        delta_power,
                        delta_toughness,
                        set_power,
                        set_toughness,
                        remove_all_abilities,
                        keywords,
                        triggered_abilities,
                        activated_abilities,
                        cant_attack,
                        cant_block,
                        doesnt_untap_during_untap_step,
                    } = ability
                    {
                        if !attachment_source {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: "AttachedModifier requires an Aura or Equipment source"
                                    .into(),
                            });
                        }
                        if *delta_power == 0
                            && *delta_toughness == 0
                            && set_power.is_none()
                            && set_toughness.is_none()
                            && !remove_all_abilities
                            && add_types.is_empty()
                            && keywords.is_empty()
                            && triggered_abilities.is_empty()
                            && activated_abilities.is_empty()
                            && !cant_attack
                            && !cant_block
                            && !doesnt_untap_during_untap_step
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
                        if let Some(condition) = condition {
                            condition
                                .validate()
                                .map_err(|reason| RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason,
                                })?;
                            if !triggered_abilities.is_empty()
                                || !activated_abilities.is_empty()
                                || *cant_attack
                                || *cant_block
                                || *doesnt_untap_during_untap_step
                            {
                                return Err(RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason: "conditioned AttachedModifier only supports characteristic modifiers"
                                        .into(),
                                });
                            }
                            if (*delta_power != 0
                                || *delta_toughness != 0
                                || set_power.is_some()
                                || *remove_all_abilities
                                || !keywords.is_empty())
                                && matches!(
                                    condition,
                                    crate::primitives::GameCondition::BattlefieldAggregate {
                                        aggregate: BattlefieldAggregate::TotalPower
                                            | BattlefieldAggregate::MaximumPower,
                                        ..
                                    }
                                )
                            {
                                return Err(RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason: "power-dependent conditional characteristics require CR 613.8 dependency ordering"
                                        .into(),
                                });
                            }
                        }
                        if !add_types.is_empty() {
                            add_types
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
                                    reason: "AttachedModifier cannot grant a delayed trigger"
                                        .into(),
                                });
                            }
                            granted.validate_shape().map_err(|reason| {
                                RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason,
                                }
                            })?;
                        }
                        for granted in activated_abilities {
                            granted.validate_shape().map_err(|reason| {
                                RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason,
                                }
                            })?;
                        }
                    }
                    if let StaticAbilityDef::ProhibitSpecialAction {
                        affected,
                        condition,
                        ..
                    } = ability
                    {
                        if matches!(affected, SpecialActionAffected::AttachedPermanent)
                            && !attachment_source
                        {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: "attached special-action prohibition requires an Aura or Equipment source".into(),
                            });
                        }
                        if let SpecialActionAffected::Permanents(filter) = affected {
                            filter
                                .validate_characteristic_constraints()
                                .map_err(|reason| RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason,
                                })?;
                        }
                        if let Some(condition) = condition {
                            condition
                                .validate()
                                .map_err(|reason| RegistryError::InvalidCard {
                                    id: card.id.clone(),
                                    reason,
                                })?;
                        }
                    }
                    if let StaticAbilityDef::SelfCombatRestriction {
                        cant_attack,
                        cant_block,
                    } = ability
                    {
                        if !cant_attack && !cant_block {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: "SelfCombatRestriction must prohibit attacking or blocking"
                                    .into(),
                            });
                        }
                    }
                    if matches!(ability, StaticAbilityDef::ControlsAttached) && !face.is_aura {
                        return Err(RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason: "ControlsAttached requires an Aura source".into(),
                        });
                    }
                }
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
                        .map_err(|reason| RegistryError::InvalidCard {
                            id: card.id.clone(),
                            reason,
                        })?;
                    match &ability.trigger {
                        TriggerCondition::WheneverAttachedObjectAttacks
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
                    if let Some(InterveningIf::GameCondition(condition)) =
                        ability.intervening_if.as_ref()
                    {
                        condition
                            .validate()
                            .map_err(|reason| RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
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
                    if let AdditionalCost::SacrificePermanent { filter } = cost {
                        filter
                            .validate_characteristic_constraints()
                            .map_err(|reason| RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason,
                            })?;
                        if !filter.all_terminal_filters_match(|leaf| {
                            matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
                                && leaf.controller == TargetController::You
                                && !leaf.exclude_source
                        }) {
                            return Err(RegistryError::InvalidCard {
                                id: card.id.clone(),
                                reason: "additional sacrifice cost filter requires Creature or AnyPermanent, controller: You, and may include its source".into(),
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
                    if let SpellEffectKind::CreateTokens { token, .. } = effect {
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
                        if let SpellEffectKind::CreateTokens { token, .. } = effect {
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
        Amount, BattlefieldCreatureCountFilter, CastTriggerPlayer, CountExpression, CounterKind,
        CreatureEventFilter, CreatureScopeController, CreatureScopeFilter, EffectSubject,
        EntersTappedAffected, EntersWithCountersAffected, GameCondition, InterveningIf, ManaAmount,
        PermanentTypeFilter, PlayerLifeAggregate, PlayerRecipient, PowerComparison,
        RelativePlayerSet, SpellCostModifier, SpellEffectKind, StaticAbilityDef, TargetFilter,
        TargetKind, TriggerCondition,
    };

    #[test]
    fn embedded_registry_loads() {
        CardRegistry::from_embedded().unwrap();
    }

    #[test]
    fn previous_card_result_rejects_an_incompatible_preceding_effect() {
        let card = r#"(
            id: "bad_previous_result",
            name: "Bad Previous Result",
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
            mana_cost: "{1}",
            types: ["Sorcery"],
            spell_effect: [ChooseResolutionBranch(
                selection: FirstApplicable,
                branches: [
                    (
                        label: "Paid discard",
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
                    (label: "Fallback", cost: None, requirement: Always, effects: []),
                ],
            )],
        )"#;

        let error = CardRegistry::from_chunks(&[card]).expect_err("missing discard cost");
        assert!(error.to_string().contains("compatible card cost"));
    }

    #[test]
    fn issue_125_damage_spells_share_their_target_with_the_death_replacement() {
        let registry = CardRegistry::from_embedded().expect("embedded registry");
        for (id, expected_damage, expected_partial) in [
            ("lava_coil", 4, None),
            (
                "scorching_dragonfire",
                3,
                Some("planeswalkers are not modeled as damage targets"),
            ),
        ] {
            let card = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
            let face = card.primary_face();
            assert!(matches!(
                face.spell_effect.as_slice(),
                [
                    SpellEffectKind::DamageTarget {
                        amount: Amount::Fixed(amount),
                        target: damage_target,
                    },
                    SpellEffectKind::ExileIfWouldDieThisTurn {
                        target: replacement_target,
                    },
                ] if *amount == expected_damage
                    && damage_target.kind == TargetKind::Creature
                    && replacement_target.kind == TargetKind::Creature
            ));
            let targeting = face.targeting.as_ref().expect("explicit grouped target");
            assert_eq!(targeting.groups.len(), 1);
            assert_eq!(targeting.groups[0].min, 1);
            assert_eq!(targeting.groups[0].max, 1);
            assert_eq!(targeting.groups[0].effect_indices, [0, 1]);
            assert_eq!(card.partial.as_deref(), expected_partial);
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
    fn conditional_reductions_allow_x_and_target_count_increases() {
        let x_cost = r#"(
            id: "bad_x_reduction",
            name: "Bad X Reduction",
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
            mana_cost: "{G}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                trigger: WheneverSelfBlocksCreature(attacker: (
                    required_keywords: [Flying],
                    excluded_keywords: [Flying],
                )),
                effect: [PumpTarget(power: 1, toughness: 0, subject: Source)],
                text: "Bad.",
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
                            permanent_type: Some(PermanentTypeFilter::Creature),
                            exclude_self: true,
                            creature_filter: Some(CreatureEventFilter {
                                power: Some(PowerComparison::AtMost(2)),
                                ..Default::default()
                            }),
                        }
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
                    mana_cost: "{{1}}{{G}}",
                    types: ["Creature"],
                    power: 1,
                    toughness: 1,
                    triggered_abilities: [(
                        trigger: WheneverPermanentEntersBattlefield(
                            permanent_type: Some({permanent_type}),
                            creature_filter: Some({creature_filter}),
                        ),
                        effect: [Draw(count: 1)],
                        text: "Bad.",
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
            mana_cost: "{G}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                trigger: WhenSelfEntersBattlefield,
                effect: [LoseLife(amount: Fixed(2), who: TriggerObjectController)],
                text: "Bad.",
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
            mana_cost: "{2}",
            types: ["Artifact"],
            triggered_abilities: [(
                trigger: WheneverAttachedObjectAttacks,
                effect: [Draw(count: 1)],
                text: "Bad.",
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
            mana_cost: "{1}{U}",
            types: ["Enchantment", "Aura"],
            spell_effect: [AuraAttach(target: (kind: Creature))],
            triggered_abilities: [(
                trigger: WheneverAttachedPlayerIsAttacked,
                effect: [Draw(count: 1)],
                text: "Bad.",
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
            mana_cost: "{1}{R}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                trigger: WhenSelfEntersBattlefield,
                effect: [DamageTarget(
                    amount: 1,
                    target: (kind: Creature, controller: DefendingPlayer),
                )],
                text: "Bad.",
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
                    mana_cost: "{{1}}{{B}}",
                    types: ["Enchantment", "Aura"],
                    spell_effect: [AuraAttach(target: (kind: Creature))],
                    triggered_abilities: [(
                        trigger: WheneverAttachedObjectDies,
                        effect: [ReturnTriggeredCardFromGraveyard(
                            reference: TriggerObject,
                            controller: AbilityController,
                            entry_counters: {entry_counters},
                        )],
                        text: "Bad.",
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
            mana_cost: "{G}",
            types: ["Creature", "Test"],
            power: 1,
            toughness: 1,
            static_abilities: [ConditionalSelfModifier(
                condition: ActivePlayer(players: Controller),
            )],
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
            mana_cost: "{G}",
            types: ["Creature", "Test"],
            power: 1,
            toughness: 1,
            static_abilities: [ConditionalSelfModifier(
                condition: BattlefieldAggregate(
                    filter: (controllers: Controller, card_type: Some(Creature)),
                    aggregate: MaximumPower,
                    min: Some(4),
                ),
                delta_power: 1,
            )],
        )"#;
        let err = CardRegistry::from_chunks_and_tokens(&[recursive], &[]).unwrap_err();
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
            mana_cost: "{2}",
            types: ["Artifact"],
            triggered_abilities: [(
                trigger: WheneverAttachedObjectIsDealtDamage,
                effect: [Draw(count: 1)],
                text: "Bad.",
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
            mana_cost: "{2}",
            types: ["Artifact", "Equipment"],
            static_abilities: [AttachedModifier(
                condition: Some(ActivePlayer(players: Controller)),
                activated_abilities: [(
                    costs: [Tap],
                    effect: [ProduceMana(options: [(g: 1)])],
                    text: "{T}: Add {G}.",
                )],
            )],
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
            mana_cost: "{2}",
            types: ["Artifact", "Equipment"],
            static_abilities: [AttachedModifier(
                condition: Some(BattlefieldAggregate(
                    filter: (controllers: Controller, card_type: Some(Creature)),
                    aggregate: MaximumPower,
                    min: Some(4),
                )),
                keywords: [FirstStrike],
            )],
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
            mana_cost: "{2}{W}",
            types: ["Creature", "Test"],
            power: 2,
            toughness: 2,
            static_abilities: [AnthemKeyword(
                filter: (controller: YouControl),
                condition: BattlefieldAggregate(
                    filter: (controllers: Controller, card_type: Some(Creature)),
                    aggregate: MaximumPower,
                    min: Some(4),
                ),
                keyword: FirstStrike,
            )],
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
    fn token_trigger_validation_rejects_empty_text_and_effects() {
        let empty_text = r#"(
            id: "bad_token",
            name: "Bad",
            types: ["Creature", "Bad"],
            colors: [Red],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                trigger: WheneverPlayerCastsSpell(caster: Controller, spell_type: Some(Noncreature)),
                effect: [PumpTarget(power: 1, toughness: 1, subject: Source)],
                text: "",
            )],
        )"#;
        let err = CardRegistry::from_chunks_and_tokens(&[], &[empty_text]).unwrap_err();
        assert!(matches!(
            err,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("triggered ability text must not be empty")
        ));

        let empty_effect = r#"(
            id: "bad_token",
            name: "Bad",
            types: ["Creature", "Bad"],
            colors: [Red],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                trigger: WheneverPlayerCastsSpell(caster: Controller, spell_type: Some(Noncreature)),
                effect: [],
                text: "Prowess",
            )],
        )"#;
        let err = CardRegistry::from_chunks_and_tokens(&[], &[empty_effect]).unwrap_err();
        assert!(matches!(
            err,
            RegistryError::InvalidCard { reason, .. }
                if reason.contains("must contain at least one effect")
        ));
    }

    #[test]
    fn target_filter_rejects_required_and_excluded_keyword_overlap() {
        let bad = r#"(
            id: "contradictory_filter",
            name: "Contradictory Filter",
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
                    subject: EffectSubject::Chosen(TargetFilter::default_creature()),
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
            mana_cost: "{G}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                trigger: AtBeginningOfEndStep(player: Controller),
                intervening_if: Some(GameCondition(CreatureDeathsThisTurn(min: None, max: None))),
                effect: [Draw(count: 1)],
                text: "Bad.",
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
            mana_cost: "{B}",
            types: ["Sorcery"],
            spell_effect: [
                ReturnFromGraveyard(
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
            mana_cost: "{G}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [
                (
                    trigger: WhenSelfEntersBattlefield,
                    effect: [TargetPlayerGainsLife(amount: 3, target: (kind: Creature))],
                    text: "bad",
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
            mana_cost: "{R}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                trigger: WhenSelfEntersBattlefield,
                effect: [DamagePlayer(amount: 1, who: DefendingPlayer)],
                text: "bad",
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
            mana_cost: "{B}",
            types: ["Enchantment"],
            activated_abilities: [
                (
                    costs: [Mana("{1}")],
                    effect: [Draw(count: 1), LoseLife(amount: Fixed(1))],
                    text: "{1}: Draw a card and lose 1 life.",
                ),
            ],
            triggered_abilities: [
                (
                    trigger: WhenSelfEntersBattlefield,
                    effect: [GainLife(amount: 2), Draw(count: 1)],
                    text: "When this enters, gain 2 life and draw a card.",
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
            mana_cost: "{1}",
            types: ["Artifact"],
            activated_abilities: [
                (
                    costs: [Tap],
                    effect: [ProduceMana(options: [(c: 1)]), LoseLife(amount: Fixed(1))],
                    text: "{T}: Add {C}. You lose 1 life.",
                ),
                (
                    costs: [Tap],
                    effect: [ProduceMana(options: [(c: 1)])],
                    text: "{T}: Add {C}.",
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
            mana_cost: "{G}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [
                (
                    trigger: AtBeginningOfUpkeep(player: Controller),
                    effect: [PumpTarget(power: 1, toughness: 1, subject: Source)],
                    text: "At the beginning of your upkeep, this gets +1/+1.",
                ),
            ],
        )"#;
        assert!(CardRegistry::from_chunks(&[good]).is_ok());

        let bad = r#"(
            id: "self_spell",
            name: "Self Spell",
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
                (name: "A", mana_cost: "{R}", types: ["Instant"]),
                (name: "B", mana_cost: "{U}", types: ["Instant"]),
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
            faces: [(name: "Only Door", mana_cost: "{2}", types: ["Enchantment", "Room"])],
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
                (name: "Left", mana_cost: "{2}", types: ["Enchantment", "Room"]),
                (name: "Right", mana_cost: "{3}", types: ["Artifact"]),
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
            mana_cost: "",
            types: ["Land"],
        )"#;
        let b = r#"(
            id: "dupe_b",
            name: " DUPE ",
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
            mana_cost: "{W}",
            types: ["Sorcery"],
            spell_effect: [ChooseResolutionBranch(
                optional: true,
                branches: [(
                    label: "Draw",
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
            mana_cost: "{W}",
            types: ["Sorcery"],
            spell_effect: [ChooseResolutionBranch(
                chooser: EachPlayer,
                optional: true,
                branches: [(
                    label: "Gain life",
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
            mana_cost: "{1}{W}",
            types: ["Creature", "Human"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                trigger: WhenSelfEntersBattlefield,
                effect: [ChooseResolutionBranch(
                    optional: false,
                    branches: [(
                        label: "Put a counter on this",
                        cost: None,
                        requirement: EffectsApplicable,
                        effects: [PutCounters(
                            counter: PlusOnePlusOne,
                            count: 1,
                            subject: Source,
                        )],
                    )],
                )],
                text: "When this enters, make a mandatory conditional choice.",
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
    fn self_combat_restriction_requires_at_least_one_prohibition() {
        let bad = r#"(
            id: "empty_self_combat_restriction",
            name: "Empty Self Combat Restriction",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            static_abilities: [SelfCombatRestriction()],
        )"#;
        let err = CardRegistry::from_chunks(&[bad]).unwrap_err();
        assert!(matches!(
            err,
            RegistryError::InvalidCard { ref reason, .. }
                if reason.contains("must prohibit attacking or blocking")
        ));
    }

    #[test]
    fn vampire_soulcaller_uses_self_combat_restriction() {
        let registry = CardRegistry::from_embedded().expect("embedded registry");
        let face = registry
            .get("vampire_soulcaller")
            .expect("Vampire Soulcaller")
            .primary_face();
        assert!(matches!(
            face.static_abilities.as_slice(),
            [StaticAbilityDef::SelfCombatRestriction {
                cant_attack: false,
                cant_block: true,
            }]
        ));
    }

    #[test]
    fn modal_spell_deserializes_and_validates() {
        let good = r#"(
            id: "modal_test",
            name: "Modal Test",
            mana_cost: "{W}",
            types: ["Instant"],
            modal_spell: (
                min_modes: 1,
                max_modes: 2,
                modes: [
                    (label: "Gain life", effects: [GainLife(amount: 3)]),
                    (label: "Draw a card", effects: [Draw(count: 1)]),
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
    fn load_rejects_invalid_modal_spell_definitions() {
        let invalid = [
            r#"(
                id: "bad_bounds",
                name: "Bad Bounds",
                types: ["Instant"],
                modal_spell: (
                    min_modes: 2,
                    max_modes: 1,
                    modes: [(label: "Draw", effects: [Draw(count: 1)])],
                ),
            )"#,
            r#"(
                id: "empty_label",
                name: "Empty Label",
                types: ["Instant"],
                modal_spell: (
                    min_modes: 1,
                    max_modes: 1,
                    modes: [(label: " ", effects: [Draw(count: 1)])],
                ),
            )"#,
            r#"(
                id: "empty_effects",
                name: "Empty Effects",
                types: ["Instant"],
                modal_spell: (
                    min_modes: 1,
                    max_modes: 1,
                    modes: [(label: "Nothing", effects: [])],
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
            types: ["Instant"],
            spell_effect: [Draw(count: 1)],
            modal_spell: (
                min_modes: 1,
                max_modes: 1,
                modes: [(label: "Gain life", effects: [GainLife(amount: 3)])],
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
                ability,
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
            ability,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                ..
            }
        )));
        let orb = reg.get("orb_of_dreams").unwrap().primary_face();
        assert!(orb.static_abilities.iter().any(|ability| matches!(
            ability,
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
                    ability,
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
            ability,
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
        let death_condition = Some(InterveningIf::GameCondition(
            GameCondition::CreatureDeathsThisTurn {
                min: Some(1),
                max: None,
            },
        ));

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
                    count: 1,
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
                requires_any_counter: false,
                exclude_source: false,
            },
        });

        let entry_amount = |id: &str| {
            reg.get(id)
                .unwrap()
                .primary_face()
                .static_abilities
                .iter()
                .find_map(|ability| match ability {
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
                    requires_any_counter: false,
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
            mana_cost: "{R}",
            types: ["Creature"],
            power: 1,
            toughness: 1,
            triggered_abilities: [(
                trigger: WheneverSelfAttacks(minimum_other_attackers: 0),
                effect: [ChangeSourceFace(action: Flip)],
                text: "Flip it.",
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
                faces: [(name: "Creature", mana_cost: "{2}{G}", types: ["Creature"])],
            )"#,
            r#"(
                id: "spell_first_adventure",
                name: "Spell First Adventure",
                layout: Adventure,
                faces: [
                    (name: "Spell", mana_cost: "{G}", types: ["Instant"]),
                    (name: "Other Spell", mana_cost: "{1}{G}", types: ["Sorcery"]),
                ],
            )"#,
            r#"(
                id: "permanent_second_adventure",
                name: "Permanent Second Adventure",
                layout: Adventure,
                faces: [
                    (name: "Creature", mana_cost: "{2}{G}", types: ["Creature"]),
                    (name: "Other Creature", mana_cost: "{1}{G}", types: ["Creature"]),
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
                faces: [(name: "Creature", mana_cost: "{2}{G}", types: ["Creature"])],
            )"#,
            r#"(
                id: "spell_first_omen",
                name: "Spell First Omen",
                layout: Omen,
                faces: [
                    (name: "Spell", mana_cost: "{G}", types: ["Instant"]),
                    (name: "Omen", mana_cost: "{1}{G}", types: ["Sorcery", "Omen"]),
                ],
            )"#,
            r#"(
                id: "missing_omen_subtype",
                name: "Missing Omen Subtype",
                layout: Omen,
                faces: [
                    (name: "Creature", mana_cost: "{2}{G}", types: ["Creature"]),
                    (name: "Spell", mana_cost: "{1}{G}", types: ["Sorcery"]),
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
                mana_cost: "{W}",
                types: ["Enchantment", "Aura"],
                static_abilities: [AttachedModifier(keywords: [Flying])],
            )"#,
            r#"(
                id: "instant_aura_attach",
                name: "Instant Aura Attach",
                mana_cost: "{W}",
                types: ["Instant"],
                spell_effect: [AuraAttach(target: (kind: Creature))],
            )"#,
            r#"(
                id: "ordinary_enchantment_modifier",
                name: "Ordinary Enchantment Modifier",
                mana_cost: "{W}",
                types: ["Enchantment"],
                static_abilities: [AttachedModifier(keywords: [Flying])],
            )"#,
            r#"(
                id: "empty_attachment_modifier",
                name: "Empty Attachment Modifier",
                mana_cost: "{1}",
                types: ["Artifact", "Equipment"],
                static_abilities: [AttachedModifier()],
            )"#,
            r#"(
                id: "ordinary_enchantment_attached_subject",
                name: "Ordinary Enchantment Attached Subject",
                mana_cost: "{1}{U}",
                types: ["Enchantment"],
                triggered_abilities: [(
                    trigger: WhenSelfEntersBattlefield,
                    effect: [Tap(subject: AttachedObject)],
                    text: "Tap the attached object.",
                )],
            )"#,
            r#"(
                id: "player_aura_attached_object",
                name: "Player Aura Attached Object",
                mana_cost: "{1}{U}",
                types: ["Enchantment", "Aura"],
                spell_effect: [AuraAttach(target: (kind: AnyPlayer))],
                triggered_abilities: [(
                    trigger: WhenSelfEntersBattlefield,
                    effect: [Tap(subject: AttachedObject)],
                    text: "Tap the attached object.",
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
    fn load_rejects_malformed_type_additions() {
        let invalid = [
            r#"(
                id: "duplicate_added_card_type",
                name: "Duplicate Added Card Type",
                mana_cost: "{1}",
                types: ["Artifact"],
                activated_abilities: [(
                    costs: [Tap],
                    effect: [AddTypes(
                        subject: Chosen((kind: AnyPermanent)),
                        addition: (card_types: [Artifact, Artifact]),
                    )],
                    text: "Duplicate.",
                )],
            )"#,
            r#"(
                id: "duplicate_added_creature_type",
                name: "Duplicate Added Creature Type",
                mana_cost: "{W}",
                types: ["Enchantment", "Aura"],
                spell_effect: [AuraAttach(target: (kind: Creature))],
                static_abilities: [AttachedModifier(
                    add_types: (creature_types: ["Knight", "Knight"]),
                )],
            )"#,
            r#"(
                id: "blank_added_creature_type",
                name: "Blank Added Creature Type",
                mana_cost: "{W}",
                types: ["Enchantment", "Aura"],
                spell_effect: [AuraAttach(target: (kind: Creature))],
                static_abilities: [AttachedModifier(
                    add_types: (creature_types: [" "]),
                )],
            )"#,
            r#"(
                id: "empty_type_addition",
                name: "Empty Type Addition",
                mana_cost: "{1}",
                types: ["Artifact"],
                activated_abilities: [(
                    costs: [Tap],
                    effect: [AddTypes(
                        subject: Chosen((kind: AnyPermanent)),
                        addition: (),
                    )],
                    text: "Empty.",
                )],
            )"#,
            r#"(
                id: "player_type_addition",
                name: "Player Type Addition",
                mana_cost: "{1}",
                types: ["Artifact"],
                activated_abilities: [(
                    costs: [Tap],
                    effect: [AddTypes(
                        subject: Chosen((kind: AnyPlayer)),
                        addition: (card_types: [Artifact]),
                    )],
                    text: "Player.",
                )],
            )"#,
            r#"(
                id: "mixed_target_type_addition",
                name: "Mixed Target Type Addition",
                mana_cost: "{1}",
                types: ["Artifact"],
                activated_abilities: [(
                    costs: [Tap],
                    effect: [AddTypes(
                        subject: Chosen((kind: AnyTarget)),
                        addition: (card_types: [Artifact]),
                    )],
                    text: "Mixed.",
                )],
            )"#,
            r#"(
                id: "subtype_on_noncreature",
                name: "Subtype On Noncreature",
                mana_cost: "{1}",
                types: ["Artifact"],
                activated_abilities: [(
                    costs: [Tap],
                    effect: [AddTypes(
                        subject: Chosen((kind: AnyPermanent)),
                        addition: (creature_types: ["Knight"]),
                    )],
                    text: "Subtype.",
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
    fn content_hash_is_stable_and_well_formed() {
        let h = CardRegistry::content_hash();
        assert_eq!(h.len(), 16, "expected 16-char hex digest");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        // Deterministic within a build (same embedded data → same digest).
        assert_eq!(h, CardRegistry::content_hash());
    }
}
