use super::*;

pub(super) fn apply_combat_restriction(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ApplyCombatRestriction { scope, restriction } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };

    if let CombatRestrictionScope::Chosen(filter) = &scope {
        let source = TargetSourceIdentity::for_stack_item(cx.engine, cx.top);
        let affected = cx
            .targets
            .iter()
            .copied()
            .filter(|oid| {
                target_filter_legal_at_resolution(
                    cx.engine,
                    filter,
                    *oid,
                    cx.controller,
                    source,
                    cx.top.trigger_context,
                )
            })
            .collect::<Vec<_>>();
        for oid in affected {
            cx.engine.state.continuous_effects.push(ContinuousEffect {
                source_id: Some(cx.top.id),
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::CombatRestriction(restriction),
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: cx.engine.state.command_index,
            });
            let name = object_display_name(&cx.engine.state, cx.engine.registry, oid);
            cx.events.push(ev_log(format!(
                "{} applies a combat restriction to {name} until end of turn",
                cx.spell_label
            )));
        }
        return Ok(EffectOutcome::Continue);
    }

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
        CombatRestrictionScope::Chosen(_) => unreachable!("handled above"),
        CombatRestrictionScope::Matching(filter) => {
            let filter_source = cx.top.source_permanent_id.unwrap_or(cx.top.id);
            (
                AffectedScope::PermanentsMatching {
                    reference_player: cx.controller,
                    exclude: Some(filter_source),
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
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: cx.engine.state.command_index,
    });
    cx.events.push(ev_log(format!(
        "{} applies a combat restriction to {log_subject} until end of turn",
        cx.spell_label
    )));
    Ok(EffectOutcome::Continue)
}
