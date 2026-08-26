use super::*;

pub(super) fn create_tokens(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CreateTokens { token, count, who } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let item = cx.top.clone();
    let recipients = player_recipients(cx, who);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let controller = cx.controller;
    let spell_label = cx.spell_label;
    let count = engine.resolve_amount(
        &count,
        AmountContext::for_stack_item(&item, controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    if engine.create_tokens(
        TokenCreationRequest {
            token_id: &token,
            count,
            recipients,
            spell_label,
            item: &item,
        },
        events,
    )? {
        return Ok(EffectOutcome::Suspended);
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn create_attacking_tokens(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CreateAttackingTokens {
        token,
        count,
        sacrifice_at_next_end_step,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let item = cx.top.clone();
    let controller = cx.controller;
    let count = cx.engine.resolve_amount(
        &count,
        AmountContext::for_stack_item(&item, controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    if count == 0 {
        return Ok(EffectOutcome::Continue);
    }
    let options = cx.engine.legal_combat_defender_options();
    if options.is_empty() {
        return Err(EngineError::Illegal(
            "no legal defending recipient for attacking tokens",
        ));
    }
    let (entries, logs) = cx.engine.prepare_token_entries(
        TokenCreationRequest {
            token_id: &token,
            count,
            recipients: vec![controller],
            spell_label: cx.spell_label,
            item: &item,
        },
        true,
    )?;
    if options.len() == 1 {
        let defenders = vec![options[0]; entries.len()];
        if cx.engine.begin_token_entry_batch(
            item,
            entries,
            logs,
            Some(AttackingTokenBatch {
                defenders,
                sacrifice_at_next_end_step,
            }),
            cx.events,
        )? {
            return Ok(EffectOutcome::Suspended);
        }
        return Ok(EffectOutcome::Continue);
    }

    let token_label = entries
        .first()
        .and_then(|entry| entry.created.identity.as_ref())
        .map(|identity| identity.name.clone())
        .unwrap_or_else(|| "token".to_string());
    let prompt = format!(
        "Choose what {token_label} 1 of {} enters attacking ({}).",
        entries.len(),
        cx.spell_label
    );
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: controller,
                source_object_id: item.id,
                prompt_text: prompt.clone(),
                choice_kind: rv1::ChoiceKind::AttackingTokenDefender as i32,
                candidate_object_ids: Vec::new(),
                candidate_card_ids: Vec::new(),
                min: 1,
                max: 1,
                ordered: false,
                candidate_names: Vec::new(),
                candidate_server_card_ids: Vec::new(),
                unique_names: false,
                generic_mana_cost: 0,
                payment_currently_legal: false,
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                candidate_selectable: Vec::new(),
                reveal_audience: rv1::ResolutionRevealAudience::None as i32,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: Vec::new(),
                combat_defender_options: options.clone(),
            },
        )),
    });
    cx.engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: controller,
        presentation: PendingResolutionPresentation {
            source_object_id: item.id,
            candidates: Vec::new(),
            min: 1,
            max: 1,
            ordered: false,
            prompt,
            choice_kind: rv1::ChoiceKind::AttackingTokenDefender,
            unique_names: false,
        },
        continuation: ResolutionContinuation::AttackingTokenDefenders {
            stack: ParkedStackResolution::new(item),
            entries,
            logs,
            chosen_defenders: Vec::new(),
            current_options: options,
            sacrifice_at_next_end_step,
        },
    });
    Ok(EffectOutcome::Suspended)
}

pub(super) fn sacrifice_observed_objects(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    if effect != SpellEffectKind::SacrificeObservedObjects {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    }
    let Some(primary) = cx.top.trigger_context.observed_object else {
        return Err(EngineError::Illegal("delayed token cohort missing"));
    };
    let observed = cx
        .engine
        .state
        .observed_object_cohorts
        .remove(&(primary.object_id, primary.zone_change_generation))
        .ok_or(EngineError::Illegal("delayed token cohort missing"))?;
    let mut dies = Vec::new();
    let mut sacrificed = 0usize;
    for reference in observed {
        let generation = cx
            .engine
            .state
            .zone_change_generation
            .get(&reference.object_id)
            .copied()
            .unwrap_or(0);
        let Some(object) = cx.engine.state.objects.get(&reference.object_id) else {
            continue;
        };
        if object.zone != Zone::Battlefield
            || object.controller != cx.controller
            || generation != reference.zone_change_generation
        {
            continue;
        }
        let owner = object.owner;
        let source = cx.engine.trigger_source_snapshot(reference.object_id);
        let was_creature = cx
            .engine
            .characteristics(reference.object_id)
            .is_some_and(|value| value.is_creature());
        let died = sacrifice_permanent(
            &mut cx.engine.state,
            cx.engine.registry,
            reference.object_id,
        )?;
        cx.events.push(permanent_moved_event(
            &cx.engine.state,
            reference.object_id,
            owner,
            rv1::permanent_moved::Destination::Graveyard,
        ));
        if let Some(source) = source {
            dies.extend(sacrifice_events(source, was_creature, cx.controller, died));
        }
        sacrificed += 1;
    }
    cx.engine.fire_triggers(&dies);
    cx.events.push(ev_log(format!(
        "P{} sacrifices {sacrificed} delayed token(s).",
        cx.controller
    )));
    Ok(EffectOutcome::Continue)
}
