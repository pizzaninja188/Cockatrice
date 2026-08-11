use super::*;

pub(super) fn pump_target(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::PumpTarget {
        mut power,
        mut toughness,
        scale,
        subject,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let spell_label = cx.spell_label;

    if let Some(scale) = scale {
        let units = engine.resolve_amount(
            &scale.amount,
            AmountContext::for_stack_item(top, cx.controller),
        );
        let units = i32::try_from(units).unwrap_or(i32::MAX);
        power = power.saturating_add(scale.power_per_unit.saturating_mul(units));
        toughness = toughness.saturating_add(scale.toughness_per_unit.saturating_mul(units));
    }

    let tid = match subject {
        EffectSubject::Source => top
            .source_permanent_id
            .filter(|_| engine.source_is_current_object(top)),
        EffectSubject::Chosen(_) => targets.first().copied(),
    };
    if let Some(tid) = tid {
        let is_valid_target = engine
            .state
            .objects
            .get(&tid)
            .is_some_and(|t| t.zone == Zone::Battlefield)
            && engine
                .characteristics(tid)
                .is_some_and(|value| value.is_creature());
        if is_valid_target {
            let tgt = object_display_name(&engine.state, engine.registry, tid);
            engine.state.continuous_effects.push(ContinuousEffect {
                source_id: top.source_permanent_id,
                affected: AffectedScope::Single(tid),
                kind: ContinuousEffectKind::PtModify {
                    delta_power: power,
                    delta_toughness: toughness,
                },
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: engine.state.command_index,
            });
            events.push(ev_log(format!(
                "{spell_label} gives +{power}/+{toughness} to {tgt}"
            )));
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn pump_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::PumpAll {
        filter,
        power,
        toughness,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let top = cx.top;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    // CR 611.2c / 613.4: snapshot the filtered creature set as the one-shot effect resolves, then
    // represent its layer-7c modification as one UntilEndOfTurn effect per affected object.
    // A creature entering later this turn was not affected and must not inherit the pump.
    let filter_source = top.source_permanent_id.unwrap_or(top.id);
    let affected = snapshot_creature_scope(engine, &filter, controller, filter_source);
    for oid in affected {
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: Some(top.id),
            affected: AffectedScope::Single(oid),
            kind: ContinuousEffectKind::PtModify {
                delta_power: power,
                delta_toughness: toughness,
            },
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });
    }
    events.push(ev_log(format!(
        "{spell_label} gives +{power}/+{toughness} to each affected creature"
    )));

    Ok(EffectOutcome::Continue)
}

pub(super) fn grant_keywords_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::GrantKeywordsAll { filter, keywords } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let top = cx.top;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    // CR 611.2c / 613 layer 6: snapshot the filtered creature set as this one-shot effect
    // resolves. Creatures that enter or begin matching later do not acquire the keyword.
    let filter_source = top.source_permanent_id.unwrap_or(top.id);
    let affected = snapshot_creature_scope(engine, &filter, controller, filter_source);
    let kw_names: Vec<&str> = keywords.iter().map(|k| k.as_str()).collect();
    for oid in affected {
        for kw in &keywords {
            engine.state.continuous_effects.push(ContinuousEffect {
                source_id: Some(top.id),
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::Layer6AddKeyword(*kw),
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: engine.state.command_index,
            });
        }
    }
    events.push(ev_log(format!(
        "{spell_label} grants {} to each affected creature until end of turn",
        kw_names.join(", ")
    )));

    Ok(EffectOutcome::Continue)
}

pub(super) fn grant_keywords(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::GrantKeywords { subject, keywords } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let (tid, effect_source_id) = match &subject {
        EffectSubject::Source => (
            cx.top
                .source_permanent_id
                .filter(|_| cx.engine.source_is_current_object(cx.top)),
            cx.top.source_permanent_id,
        ),
        EffectSubject::Chosen(target) => {
            let tid = cx.targets.first().copied().filter(|tid| {
                target_filter_legal_at_resolution(
                    cx.engine,
                    target,
                    *tid,
                    cx.controller,
                    TargetSourceIdentity::for_stack_item(cx.engine, cx.top),
                )
            });
            (tid, Some(cx.top.id))
        }
    };
    let Some(tid) = tid else {
        return Ok(EffectOutcome::Continue);
    };

    let target_name = object_display_name(&cx.engine.state, cx.engine.registry, tid);
    let keyword_names: Vec<&str> = keywords.iter().map(|keyword| keyword.as_str()).collect();
    for keyword in keywords {
        cx.engine.state.continuous_effects.push(ContinuousEffect {
            source_id: effect_source_id,
            affected: AffectedScope::Single(tid),
            kind: ContinuousEffectKind::Layer6AddKeyword(keyword),
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: cx.engine.state.command_index,
        });
    }
    cx.events.push(ev_log(format!(
        "{} grants {} to {target_name} until end of turn",
        cx.spell_label,
        keyword_names.join(", ")
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn grant_keywords_all_permanents(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::GrantKeywordsAllPermanents { filter, keywords } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };

    // Snapshot the affected permanents now. A permanent entering later in the turn was not
    // affected by this resolving one-shot effect (CR 611.2c).
    let mut affected = battlefield_objects_matching(cx.engine, &filter);
    match filter.controller {
        TargetController::Any => {}
        TargetController::You => affected.retain(|oid| {
            cx.engine
                .characteristics(*oid)
                .is_some_and(|value| value.controller == cx.controller)
        }),
        TargetController::Opponent => {
            return Err(EngineError::Illegal(
                "opponent scope is not supported by GrantKeywordsAllPermanents",
            ));
        }
    }
    let keyword_names: Vec<&str> = keywords.iter().map(|keyword| keyword.as_str()).collect();
    for oid in affected {
        for keyword in &keywords {
            cx.engine.state.continuous_effects.push(ContinuousEffect {
                source_id: Some(cx.top.id),
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::Layer6AddKeyword(*keyword),
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: cx.engine.state.command_index,
            });
        }
    }
    cx.events.push(ev_log(format!(
        "{} grants {} to each affected permanent until end of turn",
        cx.spell_label,
        keyword_names.join(", ")
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn put_counters(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::PutCounters {
        counter,
        count,
        subject,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let spell_label = cx.spell_label;

    let tid = match subject {
        EffectSubject::Source => top
            .source_permanent_id
            .filter(|_| engine.source_is_current_object(top)),
        EffectSubject::Chosen(_) => targets.first().copied(),
    };
    if let Some(tid) = tid {
        let is_valid_target = engine
            .state
            .objects
            .get(&tid)
            .is_some_and(|t| t.zone == Zone::Battlefield)
            && engine
                .characteristics(tid)
                .is_some_and(|value| value.is_creature());
        if is_valid_target {
            let tgt = object_display_name(&engine.state, engine.registry, tid);
            if let Some(t) = engine.state.objects.get_mut(&tid) {
                *t.counters.entry(counter).or_insert(0) += count;
            }
            events.push(ev_log(format!(
                "{spell_label} puts {count} {} counter{} on {tgt}",
                counter_label(counter),
                if count == 1 { "" } else { "s" },
            )));
            // Annihilation / toughness-0 death are checked by the SBA pass that
            // runs after this resolution (CR 122.3, CR 704.5f).
        }
    }

    Ok(EffectOutcome::Continue)
}
