use super::*;

fn materialize_resolving_duration(
    cx: &EffectCx<'_>,
    ordinary_source_id: Option<ObjectId>,
    duration: ResolvingEffectDuration,
) -> Option<(Option<ObjectId>, EffectDuration)> {
    match duration {
        ResolvingEffectDuration::Indefinite => {
            Some((ordinary_source_id, EffectDuration::Indefinite))
        }
        ResolvingEffectDuration::UntilEndOfTurn => {
            Some((ordinary_source_id, EffectDuration::UntilEndOfTurn))
        }
        ResolvingEffectDuration::UntilControllerNextTurn => Some((
            ordinary_source_id,
            EffectDuration::UntilTurnStart(cx.controller),
        )),
        ResolvingEffectDuration::WhileSourceOnBattlefield => {
            let source_id = cx.top.source_permanent_id?;
            let source_is_current = cx
                .engine
                .state
                .objects
                .get(&source_id)
                .is_some_and(|source| source.zone == Zone::Battlefield)
                && cx
                    .engine
                    .state
                    .zone_change_generation
                    .get(&source_id)
                    .copied()
                    .unwrap_or(0)
                    == cx.top.source_zone_change;
            source_is_current.then_some((Some(source_id), EffectDuration::WhileSourceOnBattlefield))
        }
    }
}

fn resolving_duration_label(duration: &ResolvingEffectDuration) -> &'static str {
    match duration {
        ResolvingEffectDuration::Indefinite => "",
        ResolvingEffectDuration::UntilEndOfTurn => " until end of turn",
        ResolvingEffectDuration::WhileSourceOnBattlefield => {
            " for as long as its source remains on the battlefield"
        }
        ResolvingEffectDuration::UntilControllerNextTurn => " until its controller's next turn",
    }
}

pub(in crate::engine) fn materialize_resolving_modifier(
    modifier: ResolvingPermanentModifier,
) -> Vec<ContinuousEffectKind> {
    match modifier {
        ResolvingPermanentModifier::SetTypeLine(replacement) => {
            vec![ContinuousEffectKind::Layer4SetTypeLine(replacement)]
        }
        ResolvingPermanentModifier::AddTypes(addition) => {
            vec![ContinuousEffectKind::Layer4AddTypes(addition)]
        }
        ResolvingPermanentModifier::SetBasePowerToughness { power, toughness } => {
            vec![ContinuousEffectKind::Layer7bSetPt { power, toughness }]
        }
        ResolvingPermanentModifier::GrantKeywords(keywords) => keywords
            .into_iter()
            .map(ContinuousEffectKind::Layer6AddKeyword)
            .collect(),
        ResolvingPermanentModifier::GrantActivatedAbility(ability) => {
            vec![ContinuousEffectKind::GrantActivatedAbility(ability)]
        }
    }
}

pub(super) fn create_static_emblem(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CreateStaticEmblem {
        emblem_id,
        display_name,
        effects,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let object_id = cx.engine.state.next_object_id;
    cx.engine.state.next_object_id += 1;
    cx.engine.state.static_emblems.push(StaticEmblemInstance {
        object_id,
        controller: cx.controller,
        emblem_id,
        display_name: display_name.clone(),
        effects: effects.clone(),
    });
    for effect in effects {
        if let StaticEmblemEffect::CreaturePt {
            filter,
            power,
            toughness,
        } = effect
        {
            cx.engine.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: Some(object_id),
                affected: AffectedScope::CreaturesMatching {
                    reference_player: cx.controller,
                    filter,
                    exclude: None,
                },
                kind: ContinuousEffectKind::PtModify {
                    delta_power: power,
                    delta_toughness: toughness,
                },
                condition: None,
                duration: EffectDuration::Indefinite,
                timestamp: cx.engine.state.command_index,
            });
        }
    }
    cx.events
        .push(ev_log(format!("P{} gets {display_name}.", cx.controller)));
    Ok(EffectOutcome::Continue)
}

pub(super) fn pump_target(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::PumpTarget {
        power,
        toughness,
        scale,
        subject,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let shared_units = scale.as_ref().and_then(|scale| match &scale.basis {
        PtScaleBasis::Amount(amount) => Some(i64::from(
            cx.engine.resolve_amount(
                amount,
                AmountContext::for_stack_item(cx.top, cx.controller)
                    .with_previous_effect_result(cx.previous_effect_result),
            ),
        )),
        PtScaleBasis::Subject(_) => None,
    });

    let scaled_delta = |fixed: i32, per_unit: i32, units: i64| {
        i64::from(fixed)
            .saturating_add(i64::from(per_unit).saturating_mul(units))
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    };

    // Appeal to Eirdu and the one-target Giant Growth share this effect. A grouped Chosen
    // subject applies to every surviving target; Source/Triggered subjects still bind once. CR
    // 608.2h fixes every subject-relative value before any of this instruction's modifiers are
    // installed, so grouped subjects cannot observe one another's newly created effects.
    let affected = cx.resolve_battlefield_subjects(&subject);
    let adjustments = affected
        .into_iter()
        .filter_map(|tid| {
            let characteristics = cx.engine.characteristics(tid)?;
            if !characteristics.is_creature() {
                return None;
            }
            let (delta_power, delta_toughness) =
                scale.as_ref().map_or((power, toughness), |scale| {
                    let units = match scale.basis {
                        PtScaleBasis::Amount(_) => shared_units.unwrap_or(0),
                        PtScaleBasis::Subject(PowerToughnessCharacteristic::Power) => {
                            characteristics.signed_power.unwrap_or(0)
                        }
                        PtScaleBasis::Subject(PowerToughnessCharacteristic::Toughness) => {
                            characteristics.signed_toughness.unwrap_or(0)
                        }
                    };
                    (
                        scaled_delta(power, scale.power_per_unit, units),
                        scaled_delta(toughness, scale.toughness_per_unit, units),
                    )
                });
            Some((tid, delta_power, delta_toughness))
        })
        .collect::<Vec<_>>();
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let top = cx.top;
    let spell_label = cx.spell_label;
    for (tid, delta_power, delta_toughness) in adjustments {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: top.source_permanent_id,
            affected: AffectedScope::Single(tid),
            kind: ContinuousEffectKind::PtModify {
                delta_power,
                delta_toughness,
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });
        events.push(ev_log(format!(
            "{spell_label} gives {delta_power:+}/{delta_toughness:+} to {tgt}"
        )));
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
    let affected = snapshot_mass_creature_scope(
        engine,
        &filter,
        controller,
        filter_source,
        cx.targets,
        cx.target_group_indices,
    );
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

fn doubling_delta(value: i64, characteristic: &'static str) -> Result<i32, EngineError> {
    let delta = i32::try_from(value)
        .map_err(|_| EngineError::PowerToughnessNumericRange(characteristic))?;
    // PtModify stores i32 bonuses, and Qt projects the unsigned public P/T into int.
    // Check the doubled result too: a representable bonus alone can overflow that projection.
    let doubled = value
        .checked_mul(2)
        .ok_or(EngineError::PowerToughnessNumericRange(characteristic))?;
    i32::try_from(doubled).map_err(|_| EngineError::PowerToughnessNumericRange(characteristic))?;
    Ok(delta)
}

pub(super) fn double_power_toughness_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DoublePowerToughnessAll { filter } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let source = cx.top.source_permanent_id.unwrap_or(cx.top.id);
    let affected = snapshot_mass_creature_scope(
        cx.engine,
        &filter,
        cx.controller,
        source,
        cx.targets,
        cx.target_group_indices,
    );
    // CR 608.2h / 701.10b,c: fix every creature's two signed values before installing
    // any layer-7c effect. Independent instructions would observe earlier modifications.
    let mut adjustments = Vec::with_capacity(affected.len());
    for oid in affected {
        let Some(characteristics) = cx.engine.characteristics(oid) else {
            continue;
        };
        let power = doubling_delta(characteristics.signed_power.unwrap_or(0), "power")?;
        let toughness = doubling_delta(characteristics.signed_toughness.unwrap_or(0), "toughness")?;
        adjustments.push((oid, power, toughness));
    }
    for (oid, delta_power, delta_toughness) in adjustments {
        cx.engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: Some(cx.top.id),
            affected: AffectedScope::Single(oid),
            kind: ContinuousEffectKind::PtModify {
                delta_power,
                delta_toughness,
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: cx.engine.state.command_index,
        });
    }
    cx.events.push(ev_log(format!(
        "{} doubles the power and toughness of each affected creature until end of turn",
        cx.spell_label
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
    let affected = snapshot_mass_creature_scope(
        engine,
        &filter,
        controller,
        filter_source,
        cx.targets,
        cx.target_group_indices,
    );
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

pub(super) fn remove_abilities_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::RemoveAbilitiesAll { filter } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let source = cx.top.source_permanent_id.unwrap_or(cx.top.id);
    let affected = snapshot_creature_scope(cx.engine, &filter, cx.controller, source);
    for oid in &affected {
        cx.engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: Some(cx.top.id),
            affected: AffectedScope::Single(*oid),
            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: cx.engine.state.command_index,
        });
    }
    cx.events.push(ev_log(format!(
        "{} removes all abilities from {} creature(s) until end of turn",
        cx.spell_label,
        affected.len()
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn remove_all_abilities(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::RemoveAllAbilities { subject, duration } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some((object_id, ordinary_source_id)) = cx.resolve_continuous_subject(&subject) else {
        return Ok(EffectOutcome::Continue);
    };
    let Some((source_id, runtime_duration)) =
        materialize_resolving_duration(cx, ordinary_source_id, duration.clone())
    else {
        return Ok(EffectOutcome::Continue);
    };

    cx.engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id,
        affected: AffectedScope::Single(object_id),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: runtime_duration,
        timestamp: cx.engine.state.command_index,
    });
    let duration_label = resolving_duration_label(&duration);
    cx.events.push(ev_log(format!(
        "{} removes all abilities from {}{}",
        cx.spell_label,
        object_display_name(&cx.engine.state, cx.engine.registry, object_id),
        duration_label
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn apply_permanent_modifier(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ApplyPermanentModifier {
        subject,
        modifier,
        duration,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    if matches!(subject, EffectSubject::PreviousEffectObject) {
        cx.effect_result.produced_objects = cx.previous_effect_result.produced_objects.clone();
    }
    let Some((object_id, ordinary_source_id)) = cx.resolve_continuous_subject(&subject) else {
        return Ok(EffectOutcome::Continue);
    };
    let Some((source_id, runtime_duration)) =
        materialize_resolving_duration(cx, ordinary_source_id, duration.clone())
    else {
        return Ok(EffectOutcome::Continue);
    };
    let kinds = materialize_resolving_modifier(modifier);
    for kind in kinds {
        let effect = ContinuousEffect {
            trigger_grant_origin: None,
            source_id,
            affected: AffectedScope::Single(object_id),
            kind,
            condition: None,
            duration: runtime_duration.clone(),
            timestamp: cx.engine.state.command_index,
        };
        if matches!(effect.kind, ContinuousEffectKind::GrantActivatedAbility(_)) {
            cx.engine.state.add_activated_ability_grant(effect);
        } else {
            cx.engine.state.continuous_effects.push(effect);
        }
    }
    cx.events.push(ev_log(format!(
        "{} modifies {}{}",
        cx.spell_label,
        object_display_name(&cx.engine.state, cx.engine.registry, object_id),
        resolving_duration_label(&duration)
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
    let Some((tid, effect_source_id)) = cx.resolve_continuous_subject(&subject) else {
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
            .map(|keyword| {
                let label = keyword.as_str().to_string();
                ResolutionBranchDef {
                    branch_id: tricerules_card_model::ChoiceId::new(
                        tricerules_card_model::slugify(&label),
                    )
                    .expect("keyword names produce stable choice ids"),
                    presentation: tricerules_card_model::AbilityPresentation::Fallback,
                    runtime_fallback: Some(label),
                    cost: ResolutionCost::None,
                    requirement: Default::default(),
                    effects: Vec::new(),
                }
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
                    .map(|option| {
                        let label = option.choice_label().to_string();
                        ResolutionBranchDef {
                            branch_id: tricerules_card_model::ChoiceId::new(
                                tricerules_card_model::slugify(&label),
                            )
                            .expect("protection qualities produce stable choice ids"),
                            presentation: tricerules_card_model::AbilityPresentation::Fallback,
                            runtime_fallback: Some(label),
                            cost: ResolutionCost::None,
                            requirement: Default::default(),
                            effects: Vec::new(),
                        }
                    })
                    .collect();
                return super::choices::park_resolution_branches(cx, false, branches);
            }
        }
    };

    let Some((tid, effect_source_id)) = cx.resolve_continuous_subject(&subject) else {
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
    let Some((tid, effect_source_id)) = cx.resolve_continuous_subject(&subject) else {
        return Ok(EffectOutcome::Continue);
    };

    let target_name = object_display_name(&cx.engine.state, cx.engine.registry, tid);
    let ability_text = ability.fallback_text(cx.spell_label);
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
    use tricerules_card_model::primitives::{
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
            land_types: Vec::new(),
            card_types: vec![
                tricerules_card_model::primitives::PermanentTypeFilter::Land,
                tricerules_card_model::primitives::PermanentTypeFilter::Creature,
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
        tricerules_card_model::primitives::CounterKind::PlusOnePlusOne,
        count,
        super::super::continuous::CounterPlacementOrigin::Effect,
    );
    super::misc::create_delayed_trigger(
        cx,
        SpellEffectKind::CreateDelayedTrigger {
            subject: Some(EffectSubject::Chosen(Box::new(filter.clone()))),
            affected_player: None,
            ability: Box::new(TriggeredAbilityDef {
                ability_id: tricerules_card_model::AbilityId::new("earthbend_return")
                    .expect("intrinsic ability id"),
                presentation: tricerules_card_model::AbilityPresentation::Fallback,
                trigger: TriggerCondition::WhenWatchedObjectDiesOrIsExiled,
                effect: vec![SpellEffectKind::ReturnTriggeredCard {
                    reference: TriggeredCardReference::TriggerObject,
                    from: vec![EventZone::Graveyard, EventZone::Exile],
                    destination:
                        tricerules_card_model::primitives::TriggeredCardDestination::Battlefield,
                    tapped: true,
                    controller: ReturnController::AbilityController,
                    entry_counters: vec![],
                    set_types: None,
                }],
                modal: None,
                targeting: None,
                may: false,
                intervening_if: None,
                max_triggers_per_turn: None,
                triggers_only_once: false,
            }),
        },
    )?;
    cx.events.push(ev_log(format!(
        "{} earthbends {oid} for {count}.",
        cx.spell_label
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn animate_self(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    use tricerules_card_model::primitives::{
        CreatureTypeChange, PermanentTypeFilter, TypeLineAddition,
    };

    let SpellEffectKind::AnimateSelf {
        base_power,
        base_toughness,
        colors,
        creature_types,
        keywords,
        duration,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    if !matches!(
        duration,
        ResolvingEffectDuration::UntilEndOfTurn | ResolvingEffectDuration::Indefinite
    ) {
        return Err(EngineError::Illegal(
            "AnimateSelf requires a resolving-effect duration",
        ));
    }
    let Some((oid, source_id)) = cx.resolve_continuous_subject(&EffectSubject::Source) else {
        return Ok(EffectOutcome::Continue);
    };

    let mut kinds = vec![ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
        land_types: Vec::new(),
        card_types: vec![PermanentTypeFilter::Creature],
        creature_types: Vec::new(),
    })];
    match creature_types {
        CreatureTypeChange::Preserve => {}
        CreatureTypeChange::Replace(creature_types) => {
            kinds.push(ContinuousEffectKind::Layer4SetCreatureTypes(creature_types));
        }
        CreatureTypeChange::All => {
            kinds.push(ContinuousEffectKind::Layer4SetAllCreatureTypes);
        }
    }
    if let Some(colors) = colors {
        kinds.push(ContinuousEffectKind::Layer5SetColors(colors));
    }
    kinds.extend(
        keywords
            .into_iter()
            .map(ContinuousEffectKind::Layer6AddKeyword),
    );
    kinds.push(ContinuousEffectKind::Layer7bSetPt {
        power: base_power,
        toughness: base_toughness,
    });

    let Some((source_id, runtime_duration)) =
        materialize_resolving_duration(cx, source_id, duration.clone())
    else {
        return Ok(EffectOutcome::Continue);
    };
    for kind in kinds {
        cx.engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id,
            affected: AffectedScope::Single(oid),
            kind,
            condition: None,
            duration: runtime_duration.clone(),
            timestamp: cx.engine.state.command_index,
        });
    }
    let duration_label = resolving_duration_label(&duration);
    cx.events.push(ev_log(format!(
        "{} animates {oid} as a {base_power}/{base_toughness} creature{duration_label}",
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
    let Some((tid, effect_source_id)) = cx.resolve_continuous_subject(&subject) else {
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

pub(super) fn set_base_power_toughness(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::SetBasePowerToughness {
        target,
        power,
        toughness,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(tid) = cx.targets.first().copied().filter(|tid| {
        target_filter_legal_at_resolution(
            cx.engine,
            &target,
            *tid,
            cx.controller,
            TargetSourceIdentity::for_stack_item(cx.engine, cx.top),
            cx.top.trigger_context,
        )
    }) else {
        return Ok(EffectOutcome::Continue);
    };

    let source_values = cx
        .engine
        .source_power_toughness(AmountContext::for_stack_item(cx.top, cx.controller));
    let resolve = |value: BasePowerToughnessValue| match value {
        BasePowerToughnessValue::Fixed(value) => value,
        BasePowerToughnessValue::Source(PowerToughnessCharacteristic::Power) => source_values.0,
        BasePowerToughnessValue::Source(PowerToughnessCharacteristic::Toughness) => source_values.1,
    };
    let power = resolve(power);
    let toughness = resolve(toughness);
    let target_name = object_display_name(&cx.engine.state, cx.engine.registry, tid);
    cx.engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: Some(cx.top.id),
        affected: AffectedScope::Single(tid),
        kind: ContinuousEffectKind::Layer7bSetPt { power, toughness },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: cx.engine.state.command_index,
    });
    cx.events.push(ev_log(format!(
        "{} sets {target_name}'s base power and toughness to {power}/{toughness} until end of turn",
        cx.spell_label
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn set_source_base_power_to_town_count(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    if !matches!(effect, SpellEffectKind::SetSourceBasePowerToTownCount) {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    }
    let Some(source_id) = cx.top.source_permanent_id else {
        return Ok(EffectOutcome::Continue);
    };
    let source_is_current = cx
        .engine
        .state
        .objects
        .get(&source_id)
        .is_some_and(|source| source.zone == Zone::Battlefield)
        && cx
            .engine
            .state
            .zone_change_generation
            .get(&source_id)
            .copied()
            .unwrap_or(0)
            == cx.top.source_zone_change;
    if !source_is_current {
        return Ok(EffectOutcome::Continue);
    }

    let town_count = cx
        .engine
        .state
        .players
        .iter()
        .flat_map(|player| player.battlefield.iter().copied())
        .filter_map(|object_id| cx.engine.characteristics(object_id))
        .filter(|characteristics| {
            characteristics.controller == cx.controller
                && characteristics.has_type("Land")
                && characteristics.has_type("Town")
        })
        .count();
    let power = i64::try_from(town_count).unwrap_or(i64::MAX);
    let source_name = object_display_name(&cx.engine.state, cx.engine.registry, source_id);
    cx.engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: Some(source_id),
        affected: AffectedScope::Single(source_id),
        kind: ContinuousEffectKind::Layer7bSetPower { power },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: cx.engine.state.command_index,
    });
    cx.events.push(ev_log(format!(
        "{} sets {source_name}'s base power to {power} until end of turn",
        cx.spell_label
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

pub(super) fn protect_all_permanents_you_control_with_counters(
    cx: &mut EffectCx<'_>,
) -> Result<EffectOutcome, EngineError> {
    // Mutational Advantage's ruling makes the same permanent set govern both the keyword grant
    // and the following "those permanents" damage prevention. Capture exact engine identities
    // once before adding either continuous effect; later counter changes must not alter this set.
    let affected = cx
        .engine
        .state
        .players
        .iter()
        .find(|player| player.id == cx.controller)
        .into_iter()
        .flat_map(|player| player.battlefield.iter().copied())
        .filter(|oid| {
            cx.engine
                .state
                .objects
                .get(oid)
                .is_some_and(|object| object.counters.values().any(|count| *count > 0))
        })
        .collect::<Vec<_>>();

    for &oid in &affected {
        for keyword in [Keyword::Hexproof, Keyword::Indestructible] {
            cx.engine.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: Some(cx.top.id),
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::Layer6AddKeyword(keyword),
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: cx.engine.state.command_index,
            });
        }
        cx.engine.add_damage_prevention(
            Some(cx.top),
            cx.spell_label,
            DamagePreventionScope::Recipient(oid),
            DamagePreventionAmount::All,
        );
    }

    cx.events.push(ev_log(format!(
        "{} protects {} counter-bearing permanents until end of turn",
        cx.spell_label,
        affected.len()
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn double_counters(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DoubleCounters { target } = effect else {
        return Err(EngineError::Illegal("counter doubling dispatch mismatch"));
    };
    // Read every legal recipient's current bag once before the first placement (CR 608.2h).
    let cohorts = cx
        .resolve_battlefield_subjects(&EffectSubject::Chosen(Box::new(target)))
        .into_iter()
        .filter_map(|oid| {
            cx.engine.state.objects.get(&oid).map(|object| {
                let counters = object
                    .counters
                    .iter()
                    .filter_map(|(&kind, &count)| (count > 0).then_some((kind, count)))
                    .collect::<Vec<_>>();
                (oid, counters)
            })
        })
        .collect::<Vec<_>>();
    let mut counter_events = Vec::new();
    for (oid, counters) in cohorts {
        for (kind, count) in counters {
            let Some(event) = cx.engine.place_counters_with_event(
                oid,
                kind,
                count,
                false,
                super::super::continuous::CounterPlacementOrigin::Effect,
            ) else {
                continue;
            };
            let GameEvent::CountersPlaced {
                object,
                kind,
                before,
                after,
                ..
            } = &event
            else {
                unreachable!("counter placement funnel returned a different event");
            };
            let placed = after.saturating_sub(*before);
            cx.effect_result
                .counter_placements
                .push(crate::state::CounterPlacementReceipt {
                    object: *object,
                    counter: *kind,
                    count: placed,
                });
            cx.events.push(ev_log(format!(
                "{} puts {placed} {} counter(s) on {}",
                cx.spell_label,
                kind.label(),
                object_display_name(&cx.engine.state, cx.engine.registry, oid),
            )));
            counter_events.push(event);
        }
    }
    cx.engine.fire_triggers(&counter_events, cx.events);
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
    let count = cx.engine.resolve_amount(
        &count,
        AmountContext::for_stack_item(cx.top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    let subjects = cx.resolve_battlefield_subjects(&subject);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let receipts = &mut cx.effect_result.counter_placements;
    let spell_label = cx.spell_label;
    let mut counter_events = Vec::new();
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
        let Some(counter_event) = engine.place_counters_with_event(
            tid,
            counter,
            count,
            false,
            super::super::continuous::CounterPlacementOrigin::Effect,
        ) else {
            continue;
        };
        let GameEvent::CountersPlaced {
            object,
            kind,
            before,
            after,
            ..
        } = &counter_event
        else {
            unreachable!("counter placement funnel returned a different event");
        };
        receipts.push(crate::state::CounterPlacementReceipt {
            object: *object,
            counter: *kind,
            count: after.saturating_sub(*before),
        });
        counter_events.push(counter_event);
        events.push(ev_log(format!(
            "{spell_label} puts {count} {} counter{} on {tgt}",
            counter_label(counter),
            if count == 1 { "" } else { "s" },
        )));
        // Annihilation / toughness-0 death are checked by the SBA pass that
        // runs after this resolution (CR 122.3, CR 704.5f).
    }
    engine.fire_triggers(&counter_events, events);

    Ok(EffectOutcome::Continue)
}

/// CR 122 / 608.2h: put `count` counters on every creature matching the authored scope. Untargeted
/// mass sibling of [`put_counters`]; the filtered set is snapshotted as the instruction resolves so
/// a creature that later begins matching never gains counters from this one-shot effect.
pub(super) fn put_counters_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::PutCountersAll {
        counter,
        count,
        filter,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let count = cx.engine.resolve_amount(
        &count,
        AmountContext::for_stack_item(cx.top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    let filter_source = cx.top.source_permanent_id.unwrap_or(cx.top.id);
    let affected = snapshot_mass_creature_scope(
        cx.engine,
        &filter,
        cx.controller,
        filter_source,
        cx.targets,
        cx.target_group_indices,
    );
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let receipts = &mut cx.effect_result.counter_placements;
    let spell_label = cx.spell_label;
    let mut counter_events = Vec::new();
    for tid in affected {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        let Some(counter_event) = engine.place_counters_with_event(
            tid,
            counter,
            count,
            false,
            super::super::continuous::CounterPlacementOrigin::Effect,
        ) else {
            continue;
        };
        let GameEvent::CountersPlaced {
            object,
            kind,
            before,
            after,
            ..
        } = &counter_event
        else {
            unreachable!("counter placement funnel returned a different event");
        };
        receipts.push(crate::state::CounterPlacementReceipt {
            object: *object,
            counter: *kind,
            count: after.saturating_sub(*before),
        });
        counter_events.push(counter_event);
        events.push(ev_log(format!(
            "{spell_label} puts {count} {} counter{} on {tgt}",
            counter_label(counter),
            if count == 1 { "" } else { "s" },
        )));
    }
    engine.fire_triggers(&counter_events, events);

    Ok(EffectOutcome::Continue)
}

/// CR 122 / 608.2h: put `count` counters on each planeswalker the resolving ability's controller
/// currently controls. Brokers Ascendancy and Ajani Steadfast share the resolution-time type and
/// control check; Ajani's instruction sets `exclude_self` to omit its source planeswalker.
pub(super) fn put_counters_all_planeswalkers(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::PutCountersAllPlaneswalkers {
        counter,
        count,
        exclude_self,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let count = cx.engine.resolve_amount(
        &count,
        AmountContext::for_stack_item(cx.top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    let source = cx
        .top
        .source_permanent_id
        .map(|source_id| (source_id, cx.top.source_zone_change));
    let affected = cx
        .engine
        .state
        .players
        .iter()
        .flat_map(|player| player.battlefield.iter().copied())
        .filter_map(|object_id| {
            cx.engine
                .characteristics(object_id)
                .map(|characteristics| (object_id, characteristics))
        })
        .filter(|(object_id, characteristics)| {
            let is_source_object = source.is_some_and(|(source_id, source_generation)| {
                *object_id == source_id
                    && cx
                        .engine
                        .state
                        .zone_change_generation
                        .get(object_id)
                        .copied()
                        .unwrap_or(0)
                        == source_generation
            });
            characteristics.controller == cx.controller
                && characteristics.has_type("Planeswalker")
                && (!exclude_self || !is_source_object)
        })
        .map(|(object_id, _)| object_id)
        .collect::<Vec<_>>();

    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let receipts = &mut cx.effect_result.counter_placements;
    let spell_label = cx.spell_label;
    let mut counter_events = Vec::new();
    for object_id in affected {
        let target_name = object_display_name(&engine.state, engine.registry, object_id);
        let Some(counter_event) = engine.place_counters_with_event(
            object_id,
            counter,
            count,
            false,
            super::super::continuous::CounterPlacementOrigin::Effect,
        ) else {
            continue;
        };
        let GameEvent::CountersPlaced {
            object,
            kind,
            before,
            after,
            ..
        } = &counter_event
        else {
            unreachable!("counter placement funnel returned a different event");
        };
        receipts.push(crate::state::CounterPlacementReceipt {
            object: *object,
            counter: *kind,
            count: after.saturating_sub(*before),
        });
        counter_events.push(counter_event);
        events.push(ev_log(format!(
            "{spell_label} puts {count} {} counter{} on {target_name}",
            counter_label(counter),
            if count == 1 { "" } else { "s" },
        )));
    }
    engine.fire_triggers(&counter_events, events);

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

pub(super) fn change_counters(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    use tricerules_card_model::primitives::CounterSnapshotSource;
    let (subject, counters, removing) = match effect {
        SpellEffectKind::RemoveAllCounters { counter, subject } => {
            for oid in cx.resolve_battlefield_subjects(&subject) {
                let count = cx
                    .engine
                    .state
                    .objects
                    .get(&oid)
                    .map_or(0, |object| object.counter_count(counter));
                let changed = cx.engine.remove_counters(oid, counter, count);
                if changed > 0 {
                    cx.events.push(ev_log(format!(
                        "{} removes {changed} {} counter(s) from {}",
                        cx.spell_label,
                        counter.label(),
                        object_display_name(&cx.engine.state, cx.engine.registry, oid)
                    )));
                }
            }
            return Ok(EffectOutcome::Continue);
        }
        SpellEffectKind::RemoveCounters {
            counter,
            count,
            subject,
        } => (subject, BTreeMap::from([(counter, count)]), true),
        SpellEffectKind::PutCounterSnapshot { from, subject } => {
            let reference = match from {
                CounterSnapshotSource::Source => cx
                    .top
                    .source_permanent_id
                    .map(|id| (id, cx.top.source_zone_change)),
                CounterSnapshotSource::TriggerObject => cx
                    .top
                    .trigger_context
                    .observed_object
                    .map(|r| (r.object_id, r.zone_change_generation)),
            };
            let counters = reference
                .and_then(|r| cx.engine.state.last_known_counters_by_generation.get(&r))
                .cloned()
                .unwrap_or_default();
            (subject, counters, false)
        }
        _ => return Err(EngineError::Illegal("counter dispatch mismatch")),
    };
    let subjects = cx.resolve_battlefield_subjects(&subject);
    for oid in subjects {
        for (&kind, &count) in &counters {
            let changed = if removing {
                cx.engine.remove_counters(oid, kind, count)
            } else {
                cx.engine.place_counters(
                    oid,
                    kind,
                    count,
                    super::super::continuous::CounterPlacementOrigin::Effect,
                )
            };
            if changed > 0 {
                cx.events.push(ev_log(format!(
                    "{} {} {changed} {} counter(s) {} {}",
                    cx.spell_label,
                    if removing { "removes" } else { "puts" },
                    kind.label(),
                    if removing { "from" } else { "on" },
                    object_display_name(&cx.engine.state, cx.engine.registry, oid)
                )));
            }
        }
    }
    Ok(EffectOutcome::Continue)
}

#[cfg(test)]
mod growth_tests {
    use super::*;

    fn signed_fixture(pairs: &[(i64, i64)]) -> (GameEngine, Vec<ObjectId>) {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            104_910,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;
        let growth = engine.state.players[0].hand[0];
        engine.state.objects.get_mut(&growth).unwrap().card_id = "unnatural_growth".into();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            growth,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        let mut creatures = Vec::new();
        for &(power, toughness) in pairs {
            let oid = engine.state.players[0].hand[0];
            engine.state.objects.get_mut(&oid).unwrap().card_id = "grizzly_bears".into();
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                oid,
                Zone::Battlefield,
                None,
            )
            .unwrap();
            engine.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::Layer7bSetPt { power, toughness },
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 0,
            });
            creatures.push(oid);
        }
        // Negative-toughness pairs deliberately exercise the private instruction before SBA.
        // This is not a claim that such creatures survive an ordinary priority boundary.
        engine.fire_triggers(
            &[GameEvent::PhaseBegan {
                phase: rv1::PhaseId::BeginCombat,
                active_player: 0,
            }],
            &mut Vec::new(),
        );
        engine.flush_staged_triggers(&mut Vec::new());
        assert_eq!(engine.state.stack.len(), 1);
        (engine, creatures)
    }

    fn double_now(engine: &mut GameEngine) -> Result<EffectOutcome, EngineError> {
        let top = engine.state.stack.last().unwrap().clone();
        let mut events = Vec::new();
        let previous = EffectResult::default();
        let mut result = EffectResult::default();
        let mut cx = EffectCx {
            engine,
            events: &mut events,
            targets: &[],
            targets_by_role: &[],
            target_damage: &[],
            target_group_indices: &[],
            top: &top,
            controller: 0,
            affected_player: 0,
            spell_label: "Unnatural Growth",
            previous_effect_result: &previous,
            effect_result: &mut result,
            effect_index: 0,
        };
        double_power_toughness_all(
            &mut cx,
            SpellEffectKind::DoublePowerToughnessAll {
                filter: CreatureScopeFilter {
                    controller: Some(CreatureScopeController::YouControl),
                    ..Default::default()
                },
            },
        )
    }

    #[test]
    fn growth_joint_signed_pairs_and_exact_numeric_boundaries() {
        let pairs = [
            (-2, 3),
            (3, -2),
            (-2, -3),
            (0, 0),
            (1_073_741_823, -1_073_741_824),
        ];
        let (mut engine, creatures) = signed_fixture(&pairs);
        double_now(&mut engine).unwrap();
        for (oid, (power, toughness)) in creatures.into_iter().zip(pairs) {
            let c = engine.characteristics(oid).unwrap();
            assert_eq!(
                (c.signed_power, c.signed_toughness),
                (Some(power * 2), Some(toughness * 2))
            );
        }
        for value in [1_073_741_824, -1_073_741_825, i64::MAX, i64::MIN] {
            assert!(matches!(
                doubling_delta(value, "power"),
                Err(EngineError::PowerToughnessNumericRange("power"))
            ));
        }
    }

    #[test]
    fn growth_actual_trigger_keeps_negative_power_and_reuses_same_turn_boundary() {
        let (mut engine, creatures) = signed_fixture(&[(-2, 3)]);
        let pass = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
        };
        for _ in 0..2 {
            let actor = engine.state.priority_player_id();
            engine.apply_command(actor, &pass).unwrap();
        }
        let c = engine.characteristics(creatures[0]).unwrap();
        assert_eq!((c.signed_power, c.signed_toughness), (Some(-4), Some(6)));
        let turn = engine.state.turn;
        engine.fire_triggers(
            &[GameEvent::PhaseBegan {
                phase: rv1::PhaseId::BeginCombat,
                active_player: 1,
            }],
            &mut Vec::new(),
        );
        engine.flush_staged_triggers(&mut Vec::new());
        for _ in 0..2 {
            let actor = engine.state.priority_player_id();
            engine.apply_command(actor, &pass).unwrap();
        }
        assert_eq!(engine.state.turn, turn);
        let c = engine.characteristics(creatures[0]).unwrap();
        assert_eq!((c.signed_power, c.signed_toughness), (Some(-8), Some(12)));
        // Public unsigned projection remains baseline 0, independently of rules arithmetic.
        assert_eq!(c.power, Some(0));
    }

    #[test]
    fn growth_numeric_failure_restores_earlier_draw_and_all_publication_caches() {
        let (mut engine, _) = signed_fixture(&[(2, 3), (1_073_741_824, 3)]);
        engine
            .state
            .stack
            .last_mut()
            .unwrap()
            .triggered_ability
            .as_mut()
            .unwrap()
            .effect
            .insert(
                0,
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                },
            );
        engine.initial_response_batch();
        let pass = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
        };
        engine.apply_command(0, &pass).unwrap();
        let before = engine.diagnostic_snapshot().unwrap();
        let zones = engine.private_zone_cache.clone();
        let battlefield = engine.battlefield_view_cache.clone();
        let strike = engine.first_strike_step_pending_cache;
        assert!(matches!(
            engine.apply_command(1, &pass),
            Err(EngineError::PowerToughnessNumericRange(_))
        ));
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        assert!(engine.private_zone_cache == zones);
        assert!(engine.battlefield_view_cache == battlefield);
        assert_eq!(engine.first_strike_step_pending_cache, strike);
        assert!(engine.pending_spell_cast_internal.is_none());
        assert!(engine.pending_ability_activation_internal.is_none());
    }
}

#[cfg(test)]
mod issue_236_tests {
    use super::*;
    use tricerules_card_model::primitives::TypeLineAddition;

    #[test]
    fn wrenn_shaped_modifiers_compile_to_existing_layer_kinds() {
        let modifiers = vec![
            ResolvingPermanentModifier::AddTypes(TypeLineAddition {
                land_types: Vec::new(),
                card_types: vec![PermanentTypeFilter::Creature],
                creature_types: vec!["Treefolk".into()],
            }),
            ResolvingPermanentModifier::SetBasePowerToughness {
                power: 3,
                toughness: 3,
            },
            ResolvingPermanentModifier::GrantKeywords(vec![
                Keyword::Vigilance,
                Keyword::Hexproof,
                Keyword::Haste,
            ]),
        ];
        let kinds = modifiers
            .into_iter()
            .flat_map(materialize_resolving_modifier)
            .collect::<Vec<_>>();
        assert!(matches!(
            &kinds[0],
            ContinuousEffectKind::Layer4AddTypes(addition)
                if addition.card_types == [PermanentTypeFilter::Creature]
                    && addition.creature_types == ["Treefolk"]
        ));
        assert_eq!(
            kinds[1],
            ContinuousEffectKind::Layer7bSetPt {
                power: 3,
                toughness: 3,
            }
        );
        assert_eq!(
            &kinds[2..],
            [
                ContinuousEffectKind::Layer6AddKeyword(Keyword::Vigilance),
                ContinuousEffectKind::Layer6AddKeyword(Keyword::Hexproof),
                ContinuousEffectKind::Layer6AddKeyword(Keyword::Haste),
            ]
        );
    }
}
