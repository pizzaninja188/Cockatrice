use super::*;

pub(super) fn pump_target(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::PumpTarget {
        power,
        toughness,
        target,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let spell_label = cx.spell_label;

    // `Self_` is auto-bound to the source permanent (CR 115 — not a chosen target);
    // every other filter uses the player's selected target.
    let tid = if matches!(target.kind, TargetKind::Self_) {
        top.source_permanent_id
    } else {
        targets.first().copied()
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

    // CR 613.4 layer 7c, one-shot: an UntilEndOfTurn continuous effect over the
    // filtered creature set (controller resolved from the spell's controller).
    // The resolving spell is the nominal source; it does not persist as a creature,
    // so the scope drains at cleanup (UntilEndOfTurn), not at LTB.
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: Some(top.id),
        affected: resolve_anthem_scope(&filter, controller, top.id),
        kind: ContinuousEffectKind::PtModify {
            delta_power: power,
            delta_toughness: toughness,
        },
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
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

    // CR 613 layer 6, one-shot: add a Layer6AddKeyword continuous effect for each
    // granted keyword. Overrun → Trample; Trumpet Blast → First Strike; etc.
    let scope = resolve_anthem_scope(&filter, controller, top.id);
    let kw_names: Vec<&str> = keywords.iter().map(|k| k.as_str()).collect();
    for kw in keywords {
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: Some(top.id),
            affected: scope.clone(),
            kind: ContinuousEffectKind::Layer6AddKeyword(kw),
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });
    }
    events.push(ev_log(format!(
        "{spell_label} grants {} to each affected creature until end of turn",
        kw_names.join(", ")
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
        target,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let spell_label = cx.spell_label;

    // `Self_` is auto-bound to the source permanent (CR 115); any other filter
    // uses the chosen target. Counters go on a permanent on the battlefield.
    let tid = if matches!(target.kind, TargetKind::Self_) {
        top.source_permanent_id
    } else {
        targets.first().copied()
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
