use super::*;

pub(super) fn apply_combat_restriction(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ApplyCombatRestriction { scope, restriction } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };

    let (affected, effect_source_id, log_subject) = match scope {
        CombatRestrictionScope::Source => {
            let Some(oid) = cx
                .top
                .source_permanent_id
                .filter(|_| cx.engine.source_is_current_object(cx.top))
                .filter(|oid| {
                    cx.engine
                        .characteristics(*oid)
                        .is_some_and(|characteristics| characteristics.is_creature())
                })
            else {
                return Ok(EffectOutcome::Continue);
            };
            (
                AffectedScope::Single(oid),
                cx.top.source_permanent_id,
                object_display_name(&cx.engine.state, cx.engine.registry, oid),
            )
        }
        CombatRestrictionScope::Chosen(filter) => {
            let Some(oid) = cx.targets.first().copied().filter(|oid| {
                target_filter_legal_at_resolution(
                    cx.engine,
                    &filter,
                    *oid,
                    cx.controller,
                    TargetSourceIdentity::for_stack_item(cx.engine, cx.top),
                )
            }) else {
                return Ok(EffectOutcome::Continue);
            };
            (
                AffectedScope::Single(oid),
                Some(cx.top.id),
                object_display_name(&cx.engine.state, cx.engine.registry, oid),
            )
        }
        CombatRestrictionScope::Matching(filter) => {
            let filter_source = cx.top.source_permanent_id.unwrap_or(cx.top.id);
            (
                AffectedScope::PermanentsMatching {
                    reference_player: cx.controller,
                    exclude: filter.exclude_source.then_some(filter_source),
                    filter,
                },
                Some(cx.top.id),
                "each matching creature".to_string(),
            )
        }
    };

    cx.engine.state.continuous_effects.push(ContinuousEffect {
        source_id: effect_source_id,
        affected,
        kind: ContinuousEffectKind::CombatRestriction(restriction),
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: cx.engine.state.command_index,
    });
    cx.events.push(ev_log(format!(
        "{} applies a combat restriction to {log_subject} until end of turn",
        cx.spell_label
    )));
    Ok(EffectOutcome::Continue)
}
