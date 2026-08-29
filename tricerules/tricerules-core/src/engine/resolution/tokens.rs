use super::*;

pub(super) fn create_token_copies(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CreateTokenCopies { count, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let count = cx.engine.resolve_amount(
        &count,
        AmountContext::for_stack_item(cx.top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    for source in cx.targets {
        if cx
            .engine
            .create_copied_tokens(*source, count, cx.top, cx.spell_label, cx.events)?
        {
            return Ok(EffectOutcome::Suspended);
        }
    }
    Ok(EffectOutcome::Continue)
}

pub(super) fn populate(cx: &mut EffectCx<'_>) -> Result<EffectOutcome, EngineError> {
    let mut candidates: Vec<_> = cx
        .engine
        .state
        .objects
        .values()
        .filter(|object| cx.engine.can_populate_from(object.id, cx.controller))
        .map(|object| object.id)
        .collect();
    candidates.sort_unstable();
    if candidates.is_empty() {
        return Ok(EffectOutcome::Continue);
    }
    let candidate_generations = candidates
        .iter()
        .map(|oid| {
            (
                *oid,
                cx.engine
                    .state
                    .zone_change_generation
                    .get(oid)
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect();
    let prompt = format!(
        "Choose a creature token you control to populate ({}).",
        cx.spell_label
    );
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                waterbend: false,
                selection_slots: Vec::new(),
                deciding_player_id: cx.controller,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: rv1::ChoiceKind::CopySource as i32,
                candidate_object_ids: candidates.clone(),
                candidate_names: candidates
                    .iter()
                    .map(|oid| {
                        cx.engine
                            .copiable_values_for(*oid)
                            .map(|values| values.display_name)
                            .unwrap_or_default()
                    })
                    .collect(),
                min: 1,
                max: 1,
                ..Default::default()
            },
        )),
    });
    cx.engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: cx.controller,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates,
            min: 1,
            max: 1,
            ordered: false,
            prompt,
            choice_kind: rv1::ChoiceKind::CopySource,
            unique_names: false,
        },
        continuation: ResolutionContinuation::Populate {
            stack: ParkedStackResolution::new(cx.top.clone()),
            candidate_generations,
        },
    });
    Ok(EffectOutcome::Suspended)
}

impl GameEngine {
    fn can_populate_from(&self, oid: ObjectId, controller: PlayerId) -> bool {
        self.state.objects.get(&oid).is_some_and(|object| {
            object.zone == Zone::Battlefield
                && object.controller == controller
                && object.is_token()
                && self
                    .characteristics(oid)
                    .is_some_and(|face| face.is_creature())
        })
    }

    /// Cackling Counterpart and Populate share snapshot, minting and CR 616 entry ordering.
    fn create_copied_tokens(
        &mut self,
        source: ObjectId,
        count: u32,
        item: &StackItem,
        label: &str,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(object) = self
            .state
            .objects
            .get(&source)
            .filter(|object| object.zone == Zone::Battlefield)
        else {
            return Ok(false);
        };
        // #46 explicitly defers CR 707.8a double-faced token construction. Never silently
        // flatten a physical DFC. A single-faced Clone's installed snapshot is supported.
        if !object.face_down
            && object.copiable_values.is_none()
            && self
                .registry
                .get(&object.card_id)
                .is_some_and(|definition| {
                    matches!(definition.layout, Layout::Transform | Layout::ModalDfc)
                })
        {
            events.push(ev_log(
                "Token copy not created: double-faced tokens are not implemented.".into(),
            ));
            return Ok(false);
        }
        let token_id = if object.face_down {
            "anonymous_creature_token".to_string()
        } else {
            object.card_id.clone()
        };
        let values = self
            .copiable_values_for(source)
            .ok_or(EngineError::Illegal("missing token copy values"))?;
        self.create_tokens(
            TokenCreationRequest {
                token_id: &token_id,
                values: Some(&values),
                count,
                recipients: vec![item.controller],
                spell_label: label,
                item,
            },
            false,
            None,
            events,
        )
    }

    pub(in crate::engine) fn finish_populate_choice(
        &mut self,
        pending: PendingResolution,
        chosen: ObjectId,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::Populate {
            stack,
            candidate_generations,
        } = &pending.continuation
        else {
            return Err(EngineError::Illegal("not a Populate choice"));
        };
        let generation = self
            .state
            .zone_change_generation
            .get(&chosen)
            .copied()
            .unwrap_or(0);
        if !candidate_generations.contains(&(chosen, generation))
            || !self.can_populate_from(chosen, pending.deciding_player)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("stale Populate source"));
        }
        let stack = stack.clone();
        let mut events = Vec::new();
        if self.create_copied_tokens(chosen, 1, &stack.item, "Populate", &mut events)? {
            if let Some(resolution) = self.state.pending_resolution.as_mut() {
                if let Some(parked) = resolution.continuation.stack_mut() {
                    parked.resume_effect_index = stack.resume_effect_index;
                    parked.previous_result = stack.previous_result;
                }
            }
            return Ok(super::super::events::finish_with_events(self, events));
        }
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            stack.previous_result,
            events,
        )
    }
}

pub(super) fn create_tokens(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CreateTokens {
        token,
        count,
        who,
        tapped,
        sacrifice_timing,
    } = effect
    else {
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
            values: None,
            count,
            recipients,
            spell_label,
            item: &item,
        },
        tapped,
        sacrifice_timing,
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
        sacrifice_timing,
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
            values: None,
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
            Some(AttackingTokenBatch { defenders }),
            sacrifice_timing,
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
                waterbend: false,
                selection_slots: Vec::new(),
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
            delayed_sacrifice: sacrifice_timing,
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
    let zone_snapshot = cx.engine.snapshot_zone_event();
    let mut dies = Vec::new();
    let mut sacrificed = 0usize;
    let mut departures = Vec::new();
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
        departures.push((reference, owner, source, was_creature));
    }
    // The cohort is one simultaneous instruction. Its first departure may remove a static
    // type-granting effect, so all sacrifice snapshots must precede the first move.
    for (reference, owner, source, was_creature) in departures {
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
    cx.engine.fire_zone_triggers(zone_snapshot, dies);
    cx.events.push(ev_log(format!(
        "P{} sacrifices {sacrificed} delayed token(s).",
        cx.controller
    )));
    Ok(EffectOutcome::Continue)
}
