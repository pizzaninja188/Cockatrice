use super::*;

/// Shared pure materialization for normal statics and CR 614.12 entrant-owned statics.
pub(in crate::engine) fn materialize_early_static_components(
    source: ObjectId,
    controller: PlayerId,
    generation: u64,
    timestamp: u64,
    definition: &AbilityDefinitionId,
    ability: &StaticAbilityDef,
) -> Vec<ContinuousEffect> {
    let mut components = Vec::new();
    let mut emit = |affected, kind, condition| {
        components.push(ContinuousEffect {
            source_id: Some(source),
            affected,
            kind,
            condition,
            duration: EffectDuration::WhileSourceOnBattlefield,
            timestamp,
            trigger_grant_origin: Some(TriggerAbilityOrigin::StaticGrant {
                source_id: source,
                source_zone_change: generation,
                definition: definition.clone(),
            }),
        })
    };
    match ability {
        StaticAbilityDef::AddTypesToPermanents { filter, addition } => emit(
            AffectedScope::PermanentsMatching {
                reference_player: controller,
                filter: Box::new(filter.clone()),
                exclude: None,
            },
            ContinuousEffectKind::Layer4AddTypes(addition.clone()),
            None,
        ),
        StaticAbilityDef::AttachedModifier {
            set_name,
            add_types,
            set_types,
            set_colors,
            condition,
            ..
        } => {
            let affected = AffectedScope::AttachedTo(source);
            if let Some(name) = set_name {
                emit(
                    affected.clone(),
                    ContinuousEffectKind::Layer3SetName(name.clone()),
                    condition.clone(),
                );
            }
            if !add_types.is_empty() {
                emit(
                    affected.clone(),
                    ContinuousEffectKind::Layer4AddTypes(add_types.clone()),
                    condition.clone(),
                );
            }
            if let Some(types) = set_types {
                emit(
                    affected.clone(),
                    ContinuousEffectKind::Layer4SetTypeLine(types.as_ref().clone()),
                    condition.clone(),
                );
            }
            if let Some(colors) = set_colors {
                emit(
                    affected,
                    ContinuousEffectKind::Layer5SetColors(colors.clone()),
                    condition.clone(),
                );
            }
        }
        StaticAbilityDef::ConditionalSelfModifier {
            set_types,
            add_types,
            condition,
            ..
        } => {
            let affected = AffectedScope::Single(source);
            if let Some(types) = set_types {
                emit(
                    affected.clone(),
                    ContinuousEffectKind::Layer4SetTypeLine(types.clone()),
                    Some(condition.clone()),
                );
            }
            if !add_types.is_empty() {
                emit(
                    affected,
                    ContinuousEffectKind::Layer4AddTypes(add_types.clone()),
                    Some(condition.clone()),
                );
            }
        }
        _ => {}
    }
    components
}
