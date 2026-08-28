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
            AmountContext::for_stack_item(top, cx.controller)
                .with_previous_effect_result(cx.previous_effect_result),
        );
        let units = i32::try_from(units).unwrap_or(i32::MAX);
        power = power.saturating_add(scale.power_per_unit.saturating_mul(units));
        toughness = toughness.saturating_add(scale.toughness_per_unit.saturating_mul(units));
    }

    // Appeal to Eirdu and the one-target Giant Growth share this effect. A grouped Chosen
    // subject applies to every surviving target; Source/Triggered subjects still bind once.
    let affected = if matches!(subject, EffectSubject::Chosen(_)) {
        targets.to_vec()
    } else {
        resolve_effect_subject(engine, top, targets, &subject)
            .into_iter()
            .collect()
    };
    for tid in affected {
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
                trigger_grant_origin: None,
                source_id: top.source_permanent_id,
                affected: AffectedScope::Single(tid),
                kind: ContinuousEffectKind::PtModify {
                    delta_power: power,
                    delta_toughness: toughness,
                },
                condition: None,
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
            trigger_grant_origin: None,
            source_id: Some(top.id),
            affected: AffectedScope::Single(oid),
            kind: ContinuousEffectKind::PtModify {
                delta_power: power,
                delta_toughness: toughness,
            },
            condition: None,
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
                trigger_grant_origin: None,
                source_id: Some(top.id),
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::Layer6AddKeyword(*kw),
                condition: None,
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
        EffectSubject::AttachedObject | EffectSubject::TriggerObject => (
            resolve_effect_subject(cx.engine, cx.top, cx.targets, &subject),
            if matches!(subject, EffectSubject::AttachedObject) {
                cx.top.source_permanent_id
            } else {
                Some(cx.top.id)
            },
        ),
        EffectSubject::Chosen(target) => {
            let tid = cx.targets.first().copied().filter(|tid| {
                target_filter_legal_at_resolution(
                    cx.engine,
                    target,
                    *tid,
                    cx.controller,
                    TargetSourceIdentity::for_stack_item(cx.engine, cx.top),
                    cx.top.trigger_context,
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
            trigger_grant_origin: None,
            source_id: effect_source_id,
            affected: AffectedScope::Single(tid),
            kind: ContinuousEffectKind::Layer6AddKeyword(keyword),
            condition: None,
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

pub(super) fn grant_keyword_choice(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::GrantKeywordChoice { subject, choices } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    if let Some(choice) = cx
        .top
        .resolution_branch_choices
        .get(&cx.effect_index)
        .copied()
        .flatten()
    {
        let keyword = *choices
            .get(choice)
            .ok_or(EngineError::Illegal("keyword choice became stale"))?;
        grant_keywords(
            cx,
            SpellEffectKind::GrantKeywords {
                subject,
                keywords: vec![keyword],
            },
        )
    } else {
        let branches = choices
            .into_iter()
            .map(|keyword| ResolutionBranchDef {
                label: keyword.as_str().to_string(),
                cost: ResolutionCost::None,
                requirement: Default::default(),
                effects: Vec::new(),
            })
            .collect();
        super::choices::park_resolution_branches(cx, false, branches)
    }
}

pub(super) fn grant_protection(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::GrantProtection {
        subject,
        protection,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };

    let quality = match protection {
        ProtectionGrant::Fixed(quality) => quality,
        ProtectionGrant::Choose(options) => {
            if let Some(choice) = cx
                .top
                .resolution_branch_choices
                .get(&cx.effect_index)
                .copied()
                .flatten()
            {
                *options
                    .get(choice)
                    .ok_or(EngineError::Illegal("protection choice became stale"))?
            } else {
                let branches = options
                    .into_iter()
                    .map(|option| ResolutionBranchDef {
                        label: option.choice_label().to_string(),
                        cost: ResolutionCost::None,
                        requirement: Default::default(),
                        effects: Vec::new(),
                    })
                    .collect();
                return super::choices::park_resolution_branches(cx, false, branches);
            }
        }
    };

    let (tid, effect_source_id) = match &subject {
        EffectSubject::Source => (
            cx.top
                .source_permanent_id
                .filter(|_| cx.engine.source_is_current_object(cx.top)),
            cx.top.source_permanent_id,
        ),
        EffectSubject::AttachedObject | EffectSubject::TriggerObject => (
            resolve_effect_subject(cx.engine, cx.top, cx.targets, &subject),
            if matches!(subject, EffectSubject::AttachedObject) {
                cx.top.source_permanent_id
            } else {
                Some(cx.top.id)
            },
        ),
        EffectSubject::Chosen(target) => {
            let tid = cx.targets.first().copied().filter(|tid| {
                target_filter_legal_at_resolution(
                    cx.engine,
                    target,
                    *tid,
                    cx.controller,
                    TargetSourceIdentity::for_stack_item(cx.engine, cx.top),
                    cx.top.trigger_context,
                )
            });
            (tid, Some(cx.top.id))
        }
    };
    let Some(tid) = tid else {
        return Ok(EffectOutcome::Continue);
    };
    let target_name = object_display_name(&cx.engine.state, cx.engine.registry, tid);
    cx.engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: effect_source_id,
        affected: AffectedScope::Single(tid),
        kind: ContinuousEffectKind::Layer6AddProtection(quality),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: cx.engine.state.command_index,
    });
    cx.events.push(ev_log(format!(
        "{} grants {} to {target_name} until end of turn",
        cx.spell_label,
        quality.label()
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn grant_triggered_ability(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::GrantTriggeredAbility { subject, ability } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let (tid, effect_source_id) = match &subject {
        EffectSubject::Source => (
            cx.top
                .source_permanent_id
                .filter(|_| cx.engine.source_is_current_object(cx.top)),
            cx.top.source_permanent_id,
        ),
        EffectSubject::AttachedObject | EffectSubject::TriggerObject => (
            resolve_effect_subject(cx.engine, cx.top, cx.targets, &subject),
            if matches!(subject, EffectSubject::AttachedObject) {
                cx.top.source_permanent_id
            } else {
                Some(cx.top.id)
            },
        ),
        EffectSubject::Chosen(target) => {
            let tid = cx.targets.first().copied().filter(|tid| {
                target_filter_legal_at_resolution(
                    cx.engine,
                    target,
                    *tid,
                    cx.controller,
                    TargetSourceIdentity::for_stack_item(cx.engine, cx.top),
                    cx.top.trigger_context,
                )
            });
            (tid, Some(cx.top.id))
        }
    };
    let Some(tid) = tid else {
        return Ok(EffectOutcome::Continue);
    };

    let target_name = object_display_name(&cx.engine.state, cx.engine.registry, tid);
    let ability_text = ability.text.clone();
    cx.engine
        .state
        .add_triggered_ability_grant(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: effect_source_id,
            affected: AffectedScope::Single(tid),
            kind: ContinuousEffectKind::GrantTriggeredAbility(ability),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: cx.engine.state.command_index,
        });
    cx.events.push(ev_log(format!(
        "{} grants \"{ability_text}\" to {target_name} until end of turn",
        cx.spell_label
    )));
    Ok(EffectOutcome::Continue)
}

/// CR 701.66a / 611.2a / 613: Badgermole and Rebellious Captives resolve one
/// inseparable action; SBAs cannot see the intermediate 0/0 before its counters.
pub(super) fn earthbend(
    cx: &mut EffectCx<'_>,
    count: Amount,
) -> Result<EffectOutcome, EngineError> {
    use tricerules_cards::primitives::{
        earthbend_target_filter, EventZone, ReturnController, TriggerCondition,
        TriggeredAbilityDef, TriggeredCardReference, TypeLineAddition,
    };
    let filter = earthbend_target_filter();
    let Some(oid) = cx.targets.first().copied().filter(|oid| {
        target_filter_legal_at_resolution(
            cx.engine,
            filter,
            *oid,
            cx.controller,
            TargetSourceIdentity::for_stack_item(cx.engine, cx.top),
            cx.top.trigger_context,
        )
    }) else {
        return Ok(EffectOutcome::Continue);
    };
    let count = cx.engine.resolve_amount(
        &count,
        AmountContext::for_stack_item(cx.top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    for kind in [
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![
                tricerules_cards::primitives::PermanentTypeFilter::Land,
                tricerules_cards::primitives::PermanentTypeFilter::Creature,
            ],
            creature_types: vec![],
        }),
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Haste),
        ContinuousEffectKind::Layer7bSetPt {
            power: 0,
            toughness: 0,
        },
    ] {
        cx.engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: Some(cx.top.id),
            affected: AffectedScope::Single(oid),
            kind,
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: cx.engine.state.command_index,
        });
    }
    cx.engine.place_counters(
        oid,
        tricerules_cards::primitives::CounterKind::PlusOnePlusOne,
        count,
    );
    super::misc::create_delayed_trigger(cx, SpellEffectKind::CreateDelayedTrigger {
        subject: EffectSubject::Chosen(Box::new(filter.clone())),
        ability: Box::new(TriggeredAbilityDef {
            trigger: TriggerCondition::WhenWatchedObjectDiesOrIsExiled,
            effect: vec![SpellEffectKind::ReturnTriggeredCard {
                reference: TriggeredCardReference::TriggerObject,
                from: vec![EventZone::Graveyard, EventZone::Exile],
                tapped: true, controller: ReturnController::AbilityController, entry_counters: vec![],
            }],
            modal: None, targeting: None,
            text: "When that land dies or is put into exile, return it to the battlefield tapped under your control.".into(),
            may: false, intervening_if: None, max_triggers_per_turn: None, triggers_only_once: false,
        }),
    })?;
    cx.events.push(ev_log(format!(
        "{} earthbends {oid} for {count}.",
        cx.spell_label
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn add_types(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::AddTypes { subject, addition } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let (tid, effect_source_id) = match &subject {
        EffectSubject::Source => (
            cx.top
                .source_permanent_id
                .filter(|_| cx.engine.source_is_current_object(cx.top)),
            cx.top.source_permanent_id,
        ),
        EffectSubject::AttachedObject | EffectSubject::TriggerObject => (
            resolve_effect_subject(cx.engine, cx.top, cx.targets, &subject),
            if matches!(subject, EffectSubject::AttachedObject) {
                cx.top.source_permanent_id
            } else {
                Some(cx.top.id)
            },
        ),
        EffectSubject::Chosen(target) => {
            let tid = cx.targets.first().copied().filter(|tid| {
                target_filter_legal_at_resolution(
                    cx.engine,
                    target,
                    *tid,
                    cx.controller,
                    TargetSourceIdentity::for_stack_item(cx.engine, cx.top),
                    cx.top.trigger_context,
                )
            });
            (tid, Some(cx.top.id))
        }
    };
    let Some(tid) = tid else {
        return Ok(EffectOutcome::Continue);
    };

    let target_name = object_display_name(&cx.engine.state, cx.engine.registry, tid);
    let mut type_names: Vec<String> = addition
        .card_types
        .iter()
        .map(|card_type| card_type.as_str().to_string())
        .collect();
    type_names.extend(addition.creature_types.iter().cloned());
    cx.engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: effect_source_id,
        affected: AffectedScope::Single(tid),
        kind: ContinuousEffectKind::Layer4AddTypes(addition),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: cx.engine.state.command_index,
    });
    cx.events.push(ev_log(format!(
        "{} adds {} to {target_name} until end of turn",
        cx.spell_label,
        type_names.join(", ")
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
    let affected = cx
        .engine
        .state
        .players
        .iter()
        .flat_map(|player| player.battlefield.iter().copied())
        .filter(|oid| object_matches_scoped_mass_filter(cx.engine, *oid, &filter, cx.controller))
        .collect::<Vec<_>>();
    let keyword_names: Vec<&str> = keywords.iter().map(|keyword| keyword.as_str()).collect();
    for oid in affected {
        for keyword in &keywords {
            cx.engine.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: Some(cx.top.id),
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::Layer6AddKeyword(*keyword),
                condition: None,
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

    let subjects = match subject {
        EffectSubject::Chosen(_) => targets.to_vec(),
        _ => resolve_effect_subject(engine, top, targets, &subject)
            .into_iter()
            .collect(),
    };
    for tid in subjects {
        let is_on_battlefield = engine
            .state
            .objects
            .get(&tid)
            .is_some_and(|object| object.zone == Zone::Battlefield);
        if !is_on_battlefield {
            continue;
        }

        let tgt = object_display_name(&engine.state, engine.registry, tid);
        let placed = engine.place_counters(tid, counter, count);
        if placed == 0 {
            continue;
        }
        events.push(ev_log(format!(
            "{spell_label} puts {count} {} counter{} on {tgt}",
            counter_label(counter),
            if count == 1 { "" } else { "s" },
        )));
        // Annihilation / toughness-0 death are checked by the SBA pass that
        // runs after this resolution (CR 122.3, CR 704.5f).
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn can_put_counters(
    engine: &GameEngine,
    top: &StackItem,
    targets: &[ObjectId],
    subject: &EffectSubject,
) -> bool {
    resolve_effect_subject(engine, top, targets, subject)
        .is_some_and(|object_id| engine.can_receive_counters(object_id))
}
