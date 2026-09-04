use super::super::events::ev_log_private;
use super::super::presentation::{
    stack_child_presentation_ref, PresentationPath, StackPresentationSource,
};
use super::candidate_identities;
use super::*;

pub(super) fn siege_defeat(cx: &mut EffectCx<'_>) -> Result<EffectOutcome, EngineError> {
    let Some(source_id) = cx.top.source_permanent_id else {
        return Ok(EffectOutcome::Continue);
    };
    let current_generation = cx
        .engine
        .state
        .zone_change_generation
        .get(&source_id)
        .copied()
        .unwrap_or(0);
    let Some(object) = cx.engine.state.objects.get(&source_id) else {
        return Ok(EffectOutcome::Continue);
    };
    if object.zone != Zone::Battlefield
        || current_generation != cx.top.source_zone_change
        || object.counter_count(CounterKind::Defense) != 0
        || !cx
            .engine
            .characteristics(source_id)
            .is_some_and(|value| value.has_type("Battle") && value.has_type("Siege"))
    {
        return Ok(EffectOutcome::Continue);
    }
    let owner = object.owner;
    let controller = cx.top.controller;
    let card_id = object.card_id.clone();
    let label = object_display_name(&cx.engine.state, cx.engine.registry, source_id);
    let zone_snapshot = cx.engine.snapshot_zone_event();
    let leave_event = cx.engine.battlefield_leave_event(source_id);
    move_object_to_zone(
        &mut cx.engine.state,
        cx.engine.registry,
        source_id,
        Zone::Exile,
        None,
    )?;
    cx.events.push(permanent_moved_event(
        &cx.engine.state,
        source_id,
        owner,
        rv1::permanent_moved::Destination::Exile,
    ));
    cx.events
        .push(ev_log(format!("{label} is defeated and exiled.")));
    cx.engine
        .fire_zone_triggers(zone_snapshot, leave_event.into_iter().collect::<Vec<_>>());

    let Some(definition) = cx.engine.registry.get(&card_id) else {
        return Ok(EffectOutcome::Continue);
    };
    let face_index = 1usize;
    let Some(back) = definition.face(face_index) else {
        return Ok(EffectOutcome::Continue);
    };
    let exiled = TriggerObjectRef {
        object_id: source_id,
        zone_change_generation: cx
            .engine
            .state
            .zone_change_generation
            .get(&source_id)
            .copied()
            .unwrap_or(0),
        controller_at_event: controller,
    };
    let prompt = format!(
        "Cast {} transformed without paying its mana cost?",
        back.name
    );
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: controller,
                source_object_id: source_id,
                prompt_text: prompt.clone(),
                choice_kind: rv1::ChoiceKind::SiegeCast as i32,
                candidate_object_ids: vec![source_id],
                candidate_card_ids: vec![card_id],
                min: 0,
                max: 1,
                ordered: false,
                candidate_names: vec![back.name.clone()],
                candidate_server_card_ids: Vec::new(),
                unique_names: false,
                generic_mana_cost: 0,
                payment_currently_legal: true,
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                candidate_selectable: vec![true],
                reveal_audience: 0,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: Vec::new(),
                combat_defender_options: Vec::new(),
                waterbend: false,
                selection_slots: Vec::new(),
            },
        )),
    });
    cx.events.push(ev_log(prompt.clone()));
    let mut stack = ParkedStackResolution::new(cx.top.clone());
    stack.resume_effect_index = Some(cx.effect_index + 1);
    cx.engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: controller,
        presentation: PendingResolutionPresentation {
            source_object_id: source_id,
            candidates: vec![source_id],
            min: 0,
            max: 1,
            ordered: false,
            prompt,
            choice_kind: rv1::ChoiceKind::SiegeCast,
            unique_names: false,
        },
        continuation: ResolutionContinuation::SiegeCast {
            stack,
            exiled,
            face_index,
        },
    });
    Ok(EffectOutcome::Suspended)
}

pub(super) fn exile_until_source_leaves(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ExileUntilSourceLeaves { target } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(source_id) = cx.top.source_permanent_id else {
        return Ok(EffectOutcome::Continue);
    };
    let source_generation = cx
        .engine
        .state
        .zone_change_generation
        .get(&source_id)
        .copied()
        .unwrap_or(0);
    let Some(source) = cx.engine.state.objects.get(&source_id) else {
        return Ok(EffectOutcome::Continue);
    };
    // CR 610.3b: if the source left before its ETB trigger resolved, nothing is exiled.
    if source.zone != Zone::Battlefield || source_generation != cx.top.source_zone_change {
        return Ok(EffectOutcome::Continue);
    }
    let source_ref = TriggerObjectRef {
        object_id: source_id,
        zone_change_generation: source_generation,
        controller_at_event: source.controller,
    };
    let Some(target_id) = cx.targets.first().copied().filter(|object_id| {
        target_filter_legal_at_resolution(
            cx.engine,
            &target,
            *object_id,
            cx.controller,
            TargetSourceIdentity::for_stack_item(cx.engine, cx.top),
            cx.top.trigger_context,
        )
    }) else {
        return Ok(EffectOutcome::Continue);
    };
    let Some((owner, target_label)) = cx.engine.state.objects.get(&target_id).map(|object| {
        (
            object.owner,
            object_display_name(&cx.engine.state, cx.engine.registry, target_id),
        )
    }) else {
        return Ok(EffectOutcome::Continue);
    };
    let zone_snapshot = cx.engine.snapshot_zone_event();
    let leave_event = cx.engine.battlefield_leave_event(target_id);
    move_object_to_zone(
        &mut cx.engine.state,
        cx.engine.registry,
        target_id,
        Zone::Exile,
        None,
    )?;
    cx.engine
        .fire_zone_triggers(zone_snapshot, leave_event.into_iter().collect::<Vec<_>>());
    let exiled = TriggerObjectRef {
        object_id: target_id,
        zone_change_generation: cx
            .engine
            .state
            .zone_change_generation
            .get(&target_id)
            .copied()
            .unwrap_or(0),
        controller_at_event: owner,
    };
    cx.engine
        .state
        .active_event_observers
        .push(ActiveEventObserver {
            watched: source_ref,
            matcher: EventObserverMatcher::WhenWatchedObjectLeavesBattlefield,
            payload: EventObserverPayload::ReturnExiledObject { exiled },
        });
    cx.events
        .push(ev_log(format!("{} exiles {target_label}.", cx.spell_label)));
    cx.events.push(permanent_moved_event(
        &cx.engine.state,
        target_id,
        owner,
        rv1::permanent_moved::Destination::Exile,
    ));
    Ok(EffectOutcome::Continue)
}

pub(super) fn draw(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Draw { who, count } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let drawers = player_recipients(cx, who);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let top = cx.top;
    let spell_label = cx.spell_label;

    // Blue Sun's Zenith / Braingeyser: `count` may be the cast-time X.
    let count = engine.resolve_amount(
        &count,
        AmountContext::for_stack_item(top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    for drawer in drawers {
        draw_cards_for_player(engine, events, drawer, count, spell_label)?;
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn target_player_draws(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::TargetPlayerDraws { count, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(player) = cx.targets.first().copied().map(|target| target as PlayerId) else {
        return Ok(EffectOutcome::Continue);
    };
    if cx.engine.state.player_idx(player).is_some() {
        draw_cards_for_player(cx.engine, cx.events, player, count, cx.spell_label)?;
    }
    Ok(EffectOutcome::Continue)
}

pub(super) fn discard(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Discard { who, count } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let recipients = player_recipients(cx, who);
    let mut choices = Vec::new();
    for player in recipients {
        let Some(player_index) = cx.engine.state.player_idx(player) else {
            continue;
        };
        let hand = cx.engine.state.players[player_index].hand.clone();
        let required = count.min(hand.len() as u32);
        if required == 0 {
            cx.events.push(ev_log(format!(
                "P{player} has no cards to discard ({}).",
                cx.spell_label
            )));
            continue;
        }
        choices.push(PendingPlayerDiscardChoice {
            player,
            candidate_generations: hand
                .into_iter()
                .map(|object_id| {
                    (
                        object_id,
                        cx.engine
                            .state
                            .zone_change_generation
                            .get(&object_id)
                            .copied()
                            .unwrap_or(0),
                    )
                })
                .collect(),
            required,
        });
    }
    if choices.is_empty() {
        return Ok(EffectOutcome::Continue);
    }

    park_player_set_discard_choice(
        cx.engine,
        cx.events,
        ParkedStackResolution::new(cx.top.clone()),
        PendingPlayerSetDiscard {
            choices,
            current: 0,
            selections: Vec::new(),
        },
    );
    Ok(EffectOutcome::Suspended)
}

pub(in crate::engine) fn park_player_set_discard_choice(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    stack: ParkedStackResolution,
    discard: PendingPlayerSetDiscard,
) {
    let choice = &discard.choices[discard.current];
    let candidates = choice
        .candidate_generations
        .iter()
        .map(|(object_id, _)| *object_id)
        .collect::<Vec<_>>();
    let (candidate_card_ids, candidate_names) = candidate_identities(engine, &candidates);
    let prompt = format!(
        "P{}: choose {} card(s) to discard.",
        choice.player, choice.required
    );
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: choice.player,
                source_object_id: stack.item.id,
                prompt_text: prompt.clone(),
                choice_kind: custom::ChoiceKind::HandCards as i32,
                candidate_object_ids: candidates.clone(),
                candidate_card_ids,
                candidate_names,
                min: choice.required,
                max: choice.required,
                ordered: false,
                unique_names: false,
                candidate_server_card_ids: Vec::new(),
                candidate_selectable: vec![true; candidates.len()],
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                generic_mana_cost: 0,
                payment_currently_legal: false,
                reveal_audience: rv1::ResolutionRevealAudience::None as i32,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: Vec::new(),
                combat_defender_options: Vec::new(),
                waterbend: false,
                selection_slots: Vec::new(),
            },
        )),
    });
    events.push(ev_log(prompt.clone()));
    engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: choice.player,
        presentation: PendingResolutionPresentation {
            source_object_id: stack.item.id,
            candidates,
            min: choice.required,
            max: choice.required,
            ordered: false,
            unique_names: false,
            prompt,
            choice_kind: custom::ChoiceKind::HandCards,
        },
        continuation: ResolutionContinuation::PlayerSetDiscard { stack, discard },
    });
}

pub(super) fn exile_top_with_play_permission(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ExileTopWithPlayPermission {
        player,
        count,
        count_by_cast_cost,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let count = count_by_cast_cost.map_or(count, |conditional| {
        if cx.top.cast_cost_condition_matches(&conditional.condition) {
            conditional.if_selected
        } else {
            conditional.otherwise
        }
    });
    let recipients = player_recipients(cx, player);
    let spell_label = cx.spell_label.to_string();
    for recipient in recipients {
        let player_index = cx
            .engine
            .state
            .player_idx(recipient)
            .ok_or(EngineError::UnknownPlayer(recipient))?;
        let object_ids = cx.engine.state.players[player_index]
            .library
            .iter()
            .copied()
            .take(count as usize)
            .collect::<Vec<_>>();
        if object_ids.is_empty() {
            continue;
        }
        let mut card_names = Vec::new();
        for object_id in &object_ids {
            card_names.push(object_display_name(
                &cx.engine.state,
                cx.engine.registry,
                *object_id,
            ));
            move_object_to_zone(
                &mut cx.engine.state,
                cx.engine.registry,
                *object_id,
                Zone::Exile,
                None,
            )?;
            cx.events.push(permanent_moved_event_with_library_position(
                &cx.engine.state,
                *object_id,
                recipient,
                rv1::permanent_moved::Destination::Exile,
                0,
            ));
            cx.effect_result.cards.push(payment::card_result_entry(
                &cx.engine.state,
                cx.engine.registry,
                CardResultAction::Exile,
                recipient,
                *object_id,
            ));
        }
        cx.engine.grant_exile_play_permission_group(
            recipient,
            &object_ids,
            &spell_label,
            crate::state::ExilePlayPermissionGrant::printed(
                ExilePlayPermissionScope::PlayCard,
                true,
            ),
        )?;
        cx.events.push(ev_log(format!(
            "P{recipient} exiles {} and may play those cards until the end of their next turn ({spell_label}).",
            card_names.join(", ")
        )));
    }
    Ok(EffectOutcome::Continue)
}

pub(in crate::engine) fn draw_cards_for_player(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    drawer: PlayerId,
    count: u32,
    spell_label: &str,
) -> Result<(), EngineError> {
    let idx = engine
        .state
        .player_idx(drawer)
        .ok_or(EngineError::Illegal("draw recipient not found"))?;
    // CR 120.3 / 104.3c: drawing from an empty library does NOT fail the spell —
    // draw as many as possible, then the player loses as a state-based action
    // (swept in by `sweep_life`). Aborting resolution here would corrupt state
    // (cards already drawn, stack already popped).
    let mut drawn = 0u32;
    let mut decked_out = false;
    for _ in 0..count {
        if engine.state.players[idx].library.is_empty() {
            decked_out = true;
            break;
        }
        draw_card(&mut engine.state.players[idx], &mut engine.state.objects)?;
        engine.fire_card_drawn(drawer);
        drawn += 1;
    }
    let noun = if drawn == 1 { "card" } else { "cards" };
    events.push(ev_log(format!(
        "P{drawer} draws {drawn} {noun} ({spell_label})."
    )));
    if decked_out {
        engine.state.players[idx].has_lost = true;
        events.push(ev_log(format!(
            "P{drawer} tried to draw from an empty library and loses (CR 104.3c)."
        )));
    }

    Ok(())
}

pub(super) fn exile(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Exile { subject } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let tid = resolve_zone_effect_subject(cx.engine, cx.top, cx.targets, &subject);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let spell_label = cx.spell_label;

    if let Some(tid) = tid {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        let owner = engine.state.objects.get(&tid).map(|o| o.owner);
        let zone_snapshot = engine.snapshot_zone_event();
        let leave_event = engine.battlefield_leave_event(tid);
        move_object_to_zone(&mut engine.state, engine.registry, tid, Zone::Exile, None)?;
        engine.fire_zone_triggers(zone_snapshot, leave_event.into_iter().collect::<Vec<_>>());
        events.push(ev_log(format!("{spell_label} exiles {tgt}")));
        if let Some(owner_id) = owner {
            events.push(permanent_moved_event(
                &engine.state,
                tid,
                owner_id,
                rv1::permanent_moved::Destination::Exile,
            ));
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn exile_with_owner_cast_permission(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ExileWithOwnerCastPermission {
        subject,
        alternative_cost,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(object_id) = resolve_zone_effect_subject(cx.engine, cx.top, cx.targets, &subject)
    else {
        return Ok(EffectOutcome::Continue);
    };
    let Some((owner, is_token)) = cx
        .engine
        .state
        .objects
        .get(&object_id)
        .map(|object| (object.owner, object.is_token()))
    else {
        return Ok(EffectOutcome::Continue);
    };
    let label = object_display_name(&cx.engine.state, cx.engine.registry, object_id);
    let zone_snapshot = cx.engine.snapshot_zone_event();
    let leave_event = cx.engine.battlefield_leave_event(object_id);
    move_object_to_zone(
        &mut cx.engine.state,
        cx.engine.registry,
        object_id,
        Zone::Exile,
        None,
    )?;
    cx.engine
        .fire_zone_triggers(zone_snapshot, leave_event.into_iter().collect::<Vec<_>>());
    cx.events
        .push(ev_log(format!("{} exiles {label}", cx.spell_label)));
    cx.events.push(permanent_moved_event(
        &cx.engine.state,
        object_id,
        owner,
        rv1::permanent_moved::Destination::Exile,
    ));

    if !is_token
        && cx
            .engine
            .state
            .objects
            .get(&object_id)
            .is_some_and(|object| object.zone == Zone::Exile)
    {
        cx.engine.grant_exile_play_permission(
            owner,
            object_id,
            cx.spell_label,
            crate::state::ExilePlayPermissionGrant {
                scope: ExilePlayPermissionScope::CastCard,
                cast_cost: crate::state::ExilePermissionCastCost::AlternativeManaCost(
                    alternative_cost,
                ),
                origin: crate::state::ExilePlayPermissionOrigin::Effect,
                available_after_turn_instance: None,
                until_end_of_next_turn: false,
            },
        )?;
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn exile_if_would_die_this_turn(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ExileIfWouldDieThisTurn { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(&object_id) = cx.targets.first() else {
        return Ok(EffectOutcome::Continue);
    };
    if !cx
        .engine
        .state
        .objects
        .get(&object_id)
        .is_some_and(|object| object.zone == Zone::Battlefield)
    {
        return Ok(EffectOutcome::Continue);
    }
    let zone_change_generation = cx
        .engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0);
    let replacement = ActiveDeathReplacement {
        object_id,
        zone_change_generation,
    };
    if !cx
        .engine
        .state
        .death_replacement_effects
        .contains(&replacement)
    {
        cx.engine.state.death_replacement_effects.push(replacement);
    }
    Ok(EffectOutcome::Continue)
}

pub(super) fn exile_target_gain_life_equal_to_power(
    cx: &mut EffectCx<'_>,
    _effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        // CR 608: read effective power at resolution before the object leaves.
        let power = engine.effective_power(tid).unwrap_or(0);
        let owner = engine.state.objects.get(&tid).map(|o| o.owner);
        let target_controller = engine.state.objects.get(&tid).map(|o| o.controller);
        let target_controller = target_controller.unwrap_or(controller);
        let zone_snapshot = engine.snapshot_zone_event();
        let leave_event = engine.battlefield_leave_event(tid);
        move_object_to_zone(&mut engine.state, engine.registry, tid, Zone::Exile, None)?;
        engine.fire_zone_triggers(zone_snapshot, leave_event.into_iter().collect::<Vec<_>>());
        events.push(ev_log(format!("{spell_label} exiles {tgt}")));
        if let Some(owner_id) = owner {
            events.push(permanent_moved_event(
                &engine.state,
                tid,
                owner_id,
                rv1::permanent_moved::Destination::Exile,
            ));
        }
        super::life::apply_life_gain(engine, events, target_controller, power, spell_label);
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn return_to_owners_hand(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ReturnToOwnersHand { subject } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let tid = resolve_zone_effect_subject(cx.engine, cx.top, cx.targets, &subject);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let spell_label = cx.spell_label;

    if let Some(tid) = tid {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        let owner = engine.state.objects.get(&tid).map(|o| o.owner);
        // Transient battlefield state (damage, counters, tap) is reset centrally
        // by move_object_to_zone on leaving the battlefield (CR 400.7 / 121.2).
        let zone_snapshot = engine.snapshot_zone_event();
        let leave_event = engine.battlefield_leave_event(tid);
        move_object_to_zone(&mut engine.state, engine.registry, tid, Zone::Hand, None)?;
        engine.fire_zone_triggers(zone_snapshot, leave_event.into_iter().collect::<Vec<_>>());
        events.push(ev_log(format!(
            "{spell_label} returns {tgt} to its owner's hand"
        )));
        if let Some(owner_id) = owner {
            events.push(permanent_moved_event(
                &engine.state,
                tid,
                owner_id,
                rv1::permanent_moved::Destination::Hand,
            ));
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(in crate::engine) fn move_permanent_to_owners_library(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    tid: ObjectId,
    placement: LibraryPlacement,
    spell_label: &str,
) -> Result<(), EngineError> {
    let target_name = object_display_name(&engine.state, engine.registry, tid);
    let owner = engine
        .state
        .objects
        .get(&tid)
        .map(|object| object.owner)
        .ok_or(EngineError::Illegal("no target object"))?;

    let zone_snapshot = engine.snapshot_zone_event();

    let leave_event = engine.battlefield_leave_event(tid);
    move_object_to_zone(&mut engine.state, engine.registry, tid, Zone::Library, None)?;
    engine.fire_zone_triggers(zone_snapshot, leave_event.into_iter().collect::<Vec<_>>());
    let owner_idx = engine
        .state
        .player_idx(owner)
        .ok_or(EngineError::UnknownPlayer(owner))?;
    match placement {
        LibraryPlacement::Top => {
            engine.state.players[owner_idx]
                .library
                .retain(|&oid| oid != tid);
            engine.state.players[owner_idx].library.push_front(tid);
            events.push(ev_log(format!(
                "{spell_label} puts {target_name} on top of its owner's library."
            )));
        }
        LibraryPlacement::SecondFromTop => {
            engine.state.players[owner_idx]
                .library
                .retain(|&oid| oid != tid);
            let index = usize::min(1, engine.state.players[owner_idx].library.len());
            engine.state.players[owner_idx].library.insert(index, tid);
            events.push(ev_log(format!(
                "{spell_label} puts {target_name} second from the top of its owner's library."
            )));
        }
        LibraryPlacement::Bottom => {
            events.push(ev_log(format!(
                "{spell_label} puts {target_name} on the bottom of its owner's library."
            )));
        }
        LibraryPlacement::Shuffle => {
            crate::engine::shuffle_player_library_for_current_command(&mut engine.state, owner);
            events.push(ev_log(format!("P{owner} shuffles their library.")));
        }
        LibraryPlacement::OwnerChoiceTopOrBottom
        | LibraryPlacement::OwnerChoiceSecondFromTopOrBottom => {
            return Err(EngineError::Illegal(
                "owner library placement choice must be resolved before movement",
            ));
        }
    }
    events.push(permanent_moved_event(
        &engine.state,
        tid,
        owner,
        rv1::permanent_moved::Destination::Library,
    ));
    Ok(())
}

pub(super) fn put_in_owners_library(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::PutInOwnersLibrary { subject, placement } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let tid = resolve_zone_effect_subject(cx.engine, cx.top, cx.targets, &subject);
    if let Some(tid) = tid {
        if matches!(
            placement,
            LibraryPlacement::OwnerChoiceTopOrBottom
                | LibraryPlacement::OwnerChoiceSecondFromTopOrBottom
        ) {
            let Some(object) = cx.engine.state.objects.get(&tid) else {
                return Ok(EffectOutcome::Continue);
            };
            let owner = object.owner;
            let generation = cx
                .engine
                .state
                .zone_change_generation
                .get(&tid)
                .copied()
                .unwrap_or(0);
            let nonbottom_placement =
                if placement == LibraryPlacement::OwnerChoiceSecondFromTopOrBottom {
                    LibraryPlacement::SecondFromTop
                } else {
                    LibraryPlacement::Top
                };
            let nonbottom_label = if nonbottom_placement == LibraryPlacement::SecondFromTop {
                "second from the top"
            } else {
                "the top"
            };
            let prompt = format!(
                "P{owner}: put {} on {nonbottom_label} or the bottom of its owner's library.",
                object_display_name(&cx.engine.state, cx.engine.registry, tid)
            );
            cx.events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                    rv1::ResolutionChoiceRequired {
                        deciding_player_id: owner,
                        source_object_id: cx.top.id,
                        prompt_text: prompt.clone(),
                        choice_kind: custom::ChoiceKind::ResolutionBranch as i32,
                        candidate_object_ids: Vec::new(),
                        candidate_card_ids: Vec::new(),
                        // Exactly one placement branch is mandatory. These cardinalities also
                        // drive the ruled client's Decline affordance; 0/0 would falsely present
                        // this CR 608 choice as optional even though submission rejects decline.
                        min: 1,
                        max: 1,
                        ordered: false,
                        candidate_names: Vec::new(),
                        candidate_server_card_ids: Vec::new(),
                        unique_names: false,
                        generic_mana_cost: 0,
                        payment_currently_legal: false,
                        resolution_branches: vec![
                            rv1::ResolutionBranchOption {
                                branch_index: 0,
                                label: if nonbottom_placement == LibraryPlacement::SecondFromTop {
                                    "Second from top".into()
                                } else {
                                    "Top".into()
                                },
                                cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
                                cost_text: String::new(),
                                selectable: true,
                                search_zones: Vec::new(),
                                presentation: None,
                            },
                            rv1::ResolutionBranchOption {
                                branch_index: 1,
                                label: "Bottom".into(),
                                cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
                                cost_text: String::new(),
                                selectable: true,
                                search_zones: Vec::new(),
                                presentation: None,
                            },
                        ],
                        mana_cost: String::new(),
                        candidate_selectable: Vec::new(),
                        reveal_audience: 0,
                        revealed_zone_owner_player_id: None,
                        candidate_source_zones: Vec::new(),
                        combat_defender_options: Vec::new(),
                        waterbend: false,
                        selection_slots: Vec::new(),
                    },
                )),
            });
            cx.events.push(ev_log(prompt.clone()));
            cx.engine.state.pending_resolution = Some(PendingResolution {
                deciding_player: owner,
                presentation: PendingResolutionPresentation {
                    source_object_id: cx.top.id,
                    candidates: Vec::new(),
                    min: 1,
                    max: 1,
                    ordered: false,
                    prompt,
                    choice_kind: custom::ChoiceKind::ResolutionBranch,
                    unique_names: false,
                },
                continuation: ResolutionContinuation::OwnerLibraryPlacement {
                    stack: ParkedStackResolution::new(cx.top.clone()),
                    object_id: tid,
                    owner,
                    zone_change_generation: generation,
                    nonbottom_placement,
                    spell_label: cx.spell_label.to_string(),
                },
            });
            return Ok(EffectOutcome::Suspended);
        }
        move_permanent_to_owners_library(cx.engine, cx.events, tid, placement, cx.spell_label)?;
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn shuffle_permanents_into_owners_libraries(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ShufflePermanentsIntoOwnersLibraries { subjects } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };

    // CR 701.24c: the instruction still shuffles a named object's owner's library when that
    // object is no longer in the expected zone. Capture the source owner on the stack item so a
    // departed source, including a token that has ceased to exist, still identifies that library.
    let mut affected_owners = Vec::new();
    let mut object_ids = Vec::new();
    let mut chosen_role = 0usize;
    for subject in subjects {
        match subject {
            EffectSubject::Source => {
                if let Some(owner) = cx.top.source_owner {
                    if !affected_owners.contains(&owner) {
                        affected_owners.push(owner);
                    }
                }
                if cx.engine.source_is_current_object(cx.top) {
                    if let Some(object_id) = cx.top.source_permanent_id {
                        if cx
                            .engine
                            .state
                            .objects
                            .get(&object_id)
                            .is_some_and(|object| object.zone == Zone::Battlefield)
                            && !object_ids.contains(&object_id)
                        {
                            object_ids.push(object_id);
                        }
                    }
                }
            }
            EffectSubject::Chosen(_) => {
                let targets = cx
                    .targets_by_role
                    .get(chosen_role)
                    .ok_or(EngineError::Illegal(
                        "missing target role for multi-subject library shuffle",
                    ))?;
                chosen_role += 1;
                for &object_id in targets {
                    let Some(object) = cx.engine.state.objects.get(&object_id) else {
                        continue;
                    };
                    if object.zone != Zone::Battlefield {
                        continue;
                    }
                    if !affected_owners.contains(&object.owner) {
                        affected_owners.push(object.owner);
                    }
                    if !object_ids.contains(&object_id) {
                        object_ids.push(object_id);
                    }
                }
            }
            EffectSubject::AttachedObject
            | EffectSubject::TriggerObject
            | EffectSubject::PreviousEffectObject
            | EffectSubject::SearchedObject(_) => {
                return Err(EngineError::Illegal(
                    "unsupported subject for multi-subject library shuffle",
                ));
            }
        }
    }

    // Snapshot every departure before moving anything so all LTB observers see one simultaneous
    // event boundary. Generation-bound ObjectIds are deduplicated above before this snapshot.
    let zone_snapshot = cx.engine.snapshot_zone_event();
    let moves = object_ids
        .into_iter()
        .filter_map(|object_id| {
            let object = cx.engine.state.objects.get(&object_id)?;
            let owner = object.owner;
            let name = object_display_name(&cx.engine.state, cx.engine.registry, object_id);
            let leave_event = cx.engine.battlefield_leave_event(object_id);
            Some((object_id, owner, name, leave_event))
        })
        .collect::<Vec<_>>();
    let mut leave_events = Vec::new();
    for (object_id, owner, name, leave_event) in moves {
        move_object_to_zone(
            &mut cx.engine.state,
            cx.engine.registry,
            object_id,
            Zone::Library,
            None,
        )?;
        leave_events.extend(leave_event);
        cx.events.push(permanent_moved_event(
            &cx.engine.state,
            object_id,
            owner,
            rv1::permanent_moved::Destination::Library,
        ));
        cx.events.push(ev_log(format!(
            "{} puts {name} into its owner's library.",
            cx.spell_label
        )));
    }
    cx.engine.fire_zone_triggers(zone_snapshot, leave_events);

    // GameState player order is stable, so it provides deterministic shuffle ordering even when
    // subjects name permanents controlled or owned in a different order.
    let player_order = cx
        .engine
        .state
        .players
        .iter()
        .map(|player| player.id)
        .collect::<Vec<_>>();
    for owner in player_order {
        if affected_owners.contains(&owner) {
            crate::engine::shuffle_player_library_for_current_command(&mut cx.engine.state, owner);
            cx.events
                .push(ev_log(format!("P{owner} shuffles their library.")));
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn discard_cards(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DiscardCards {
        count,
        target: _,
        chooser,
        card_filter,
        optional,
        visibility,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(&target) = cx.targets.first() else {
        return Ok(EffectOutcome::Continue);
    };
    choose_hand_cards_for_player(
        cx,
        target as PlayerId,
        HandCardChoiceSpec {
            count,
            chooser,
            card_filter: card_filter.as_ref(),
            optional,
            visibility,
            draw_after: 0,
            action: HandCardAction::Discard,
        },
    )
}

pub(super) fn exile_cards_from_hand(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ExileCardsFromHand {
        count,
        target: _,
        chooser,
        card_filter,
        optional,
        visibility,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(&target) = cx.targets.first() else {
        return Ok(EffectOutcome::Continue);
    };
    choose_hand_cards_for_player(
        cx,
        target as PlayerId,
        HandCardChoiceSpec {
            count,
            chooser,
            card_filter: card_filter.as_ref(),
            optional,
            visibility,
            draw_after: 0,
            action: HandCardAction::Exile,
        },
    )
}

pub(super) fn draw_discard(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DrawDiscard {
        who,
        draw_count,
        discard_count,
        order,
        optional,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let recipients = player_recipients(cx, who);
    let [player] = recipients.as_slice() else {
        return if recipients.is_empty() {
            Ok(EffectOutcome::Continue)
        } else {
            Err(EngineError::Illegal(
                "DrawDiscard requires exactly one affected player",
            ))
        };
    };
    let player = *player;

    match order {
        DrawDiscardOrder::DrawThenDiscard => {
            draw_cards_for_player(cx.engine, cx.events, player, draw_count, cx.spell_label)?;
            choose_hand_cards_for_player(
                cx,
                player,
                HandCardChoiceSpec {
                    count: discard_count,
                    chooser: DiscardChooser::AffectedPlayer,
                    card_filter: None,
                    optional: false,
                    visibility: HandChoiceVisibility::PrivateLook,
                    draw_after: 0,
                    action: HandCardAction::Discard,
                },
            )
        }
        DrawDiscardOrder::DiscardThenDraw => choose_hand_cards_for_player(
            cx,
            player,
            HandCardChoiceSpec {
                count: discard_count,
                chooser: DiscardChooser::AffectedPlayer,
                card_filter: None,
                optional,
                visibility: HandChoiceVisibility::PrivateLook,
                draw_after: draw_count,
                action: HandCardAction::Discard,
            },
        ),
    }
}

struct HandCardChoiceSpec<'a> {
    count: u32,
    chooser: DiscardChooser,
    card_filter: Option<&'a CardTypeFilter>,
    optional: bool,
    visibility: HandChoiceVisibility,
    draw_after: u32,
    action: HandCardAction,
}

fn choose_hand_cards_for_player(
    cx: &mut EffectCx<'_>,
    affected_player: PlayerId,
    spec: HandCardChoiceSpec<'_>,
) -> Result<EffectOutcome, EngineError> {
    let HandCardChoiceSpec {
        count,
        chooser,
        card_filter,
        optional,
        visibility,
        draw_after,
        action,
    } = spec;
    let controller = cx.controller;
    let top = cx.top;
    let spell_label = cx.spell_label;
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let Some(pi) = engine.state.player_idx(affected_player) else {
        return Ok(EffectOutcome::Continue);
    };
    let hand = engine.state.players[pi].hand.clone();
    let eligible: Vec<ObjectId> = hand
        .iter()
        .copied()
        .filter(|object_id| {
            card_matches_type_filter(&engine.state, engine.registry, *object_id, card_filter)
        })
        .collect();

    if chooser == DiscardChooser::Random {
        let chosen_count = (count as usize).min(eligible.len());
        if chosen_count == 0 {
            events.push(ev_log(format!(
                "P{affected_player} has no eligible cards to {} ({spell_label}).",
                hand_action_verb(action)
            )));
        } else {
            let mut shuffled = eligible;
            shuffle_object_ids_for_current_command(&engine.state, affected_player, &mut shuffled);
            for oid in shuffled.into_iter().take(chosen_count) {
                let result = perform_hand_card_action(
                    engine,
                    events,
                    affected_player,
                    oid,
                    action,
                    spell_label,
                )?;
                cx.effect_result.cards.push(result);
            }
        }
        return Ok(EffectOutcome::Continue);
    }

    if eligible.is_empty() {
        events.push(ev_log(format!(
            "P{affected_player} has no eligible cards to {} ({spell_label}).",
            hand_action_verb(action)
        )));
        if action == HandCardAction::Discard && draw_after > 0 && !optional {
            draw_cards_for_player(engine, events, affected_player, draw_after, spell_label)?;
        }
        return Ok(EffectOutcome::Continue);
    }

    let n = (eligible.len() as u32).min(count);
    let min = if optional { 0 } else { n };
    let deciding_player = match chooser {
        DiscardChooser::AffectedPlayer => affected_player,
        DiscardChooser::Controller => controller,
        DiscardChooser::Random => unreachable!("handled above"),
    };
    let choice_kind = match chooser {
        DiscardChooser::AffectedPlayer => custom::ChoiceKind::HandCards,
        DiscardChooser::Controller => custom::ChoiceKind::OpponentHand,
        DiscardChooser::Random => unreachable!("handled above"),
    };
    let (candidate_card_ids, candidate_names) = candidate_identities(engine, &hand);
    let candidate_selectable = hand
        .iter()
        .map(|object_id| eligible.contains(object_id))
        .collect();
    let verb = hand_action_verb(action);
    let prompt = if optional {
        format!("P{deciding_player}: you may choose a card for P{affected_player} to {verb}.")
    } else {
        format!("P{deciding_player}: choose {n} card(s) for P{affected_player} to {verb}.")
    };
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: deciding_player,
                source_object_id: top.id,
                prompt_text: prompt.clone(),
                choice_kind: choice_kind as i32,
                candidate_object_ids: hand.clone(),
                candidate_card_ids,
                candidate_names,
                min,
                max: n,
                ordered: false,
                unique_names: false,
                candidate_server_card_ids: Vec::new(),
                candidate_selectable,
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                generic_mana_cost: 0,
                payment_currently_legal: false,
                reveal_audience: if visibility == HandChoiceVisibility::PublicReveal {
                    rv1::ResolutionRevealAudience::AllParticipants as i32
                } else {
                    rv1::ResolutionRevealAudience::None as i32
                },
                revealed_zone_owner_player_id: (visibility == HandChoiceVisibility::PublicReveal)
                    .then_some(affected_player),
                candidate_source_zones: Vec::new(),
                combat_defender_options: Vec::new(),
                waterbend: false,
                selection_slots: Vec::new(),
            },
        )),
    });
    events.push(ev_log(prompt.clone()));
    engine.state.pending_resolution = Some(PendingResolution {
        deciding_player,
        presentation: PendingResolutionPresentation {
            source_object_id: top.id,
            candidates: eligible.clone(),
            min,
            max: n,
            ordered: false,
            unique_names: false,
            prompt,
            choice_kind,
        },
        continuation: ResolutionContinuation::HandChoice {
            stack: ParkedStackResolution::new(top.clone()),
            hand_choice: PendingHandChoice {
                affected_player,
                action,
                candidate_generations: eligible
                    .iter()
                    .map(|object_id| {
                        (
                            *object_id,
                            engine
                                .state
                                .zone_change_generation
                                .get(object_id)
                                .copied()
                                .unwrap_or(0),
                        )
                    })
                    .collect(),
                draw_after,
                draw_only_if_discarded: optional,
            },
        },
    });
    Ok(EffectOutcome::Suspended)
}

fn hand_action_verb(action: HandCardAction) -> &'static str {
    match action {
        HandCardAction::Discard => "discard",
        HandCardAction::Exile => "exile",
    }
}

fn perform_discard_action(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    affected_player: PlayerId,
    object_id: ObjectId,
    spell_label: &str,
) -> Result<CardResultEntry, EngineError> {
    let (card_name, moved) = perform_discard(
        &mut engine.state,
        engine.registry,
        affected_player,
        object_id,
    )?;
    events.push(moved);
    events.push(ev_log(format!(
        "P{affected_player} discards {card_name} ({spell_label})."
    )));
    Ok(payment::card_result_entry(
        &engine.state,
        engine.registry,
        CardResultAction::Discard,
        affected_player,
        object_id,
    ))
}

fn perform_exile_from_hand(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    affected_player: PlayerId,
    object_id: ObjectId,
    spell_label: &str,
) -> Result<CardResultEntry, EngineError> {
    let card_name = object_display_name(&engine.state, engine.registry, object_id);
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        object_id,
        Zone::Exile,
        None,
    )?;
    events.push(permanent_moved_event(
        &engine.state,
        object_id,
        affected_player,
        rv1::permanent_moved::Destination::Exile,
    ));
    events.push(ev_log(format!(
        "P{affected_player} exiles {card_name} from their hand ({spell_label})."
    )));
    Ok(payment::card_result_entry(
        &engine.state,
        engine.registry,
        CardResultAction::Exile,
        affected_player,
        object_id,
    ))
}

pub(crate) fn perform_hand_card_action(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    affected_player: PlayerId,
    object_id: ObjectId,
    action: HandCardAction,
    spell_label: &str,
) -> Result<CardResultEntry, EngineError> {
    match action {
        HandCardAction::Discard => {
            perform_discard_action(engine, events, affected_player, object_id, spell_label)
        }
        HandCardAction::Exile => {
            perform_exile_from_hand(engine, events, affected_player, object_id, spell_label)
        }
    }
}

pub(super) fn mill_target_player(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::MillTargetPlayer { count, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let recipients = cx.targets.first().map(|target| *target as PlayerId);
    if let Some(recipient) = recipients {
        *cx.effect_result = mill_players(cx, &[recipient], count)?.into();
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn mill(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Mill { count, who } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let amount_context = AmountContext::for_stack_item(cx.top, cx.controller)
        .with_previous_effect_result(cx.previous_effect_result);
    let count = cx.engine.resolve_amount(&count, amount_context);
    let recipients = player_recipients(cx, who);
    *cx.effect_result = mill_players(cx, &recipients, count)?.into();

    Ok(EffectOutcome::Continue)
}

fn mill_players(
    cx: &mut EffectCx<'_>,
    recipients: &[PlayerId],
    count: u32,
) -> Result<CardResultCohort, EngineError> {
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let spell_label = cx.spell_label;
    let mut result = CardResultCohort::default();

    for &pid in recipients {
        let Some(pi) = engine.state.player_idx(pid) else {
            continue;
        };
        let mut milled = 0u32;
        for _ in 0..count {
            let Some(oid) = engine.state.players[pi].library.front().copied() else {
                break;
            };
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                oid,
                Zone::Graveyard,
                None,
            )?;
            events.push(permanent_moved_event(
                &engine.state,
                oid,
                pid,
                rv1::permanent_moved::Destination::Graveyard,
            ));
            result.cards.push(payment::card_result_entry(
                &engine.state,
                engine.registry,
                CardResultAction::Mill,
                pid,
                oid,
            ));
            milled += 1;
        }
        events.push(ev_log(format!(
            "{spell_label} mills {milled} card(s) from P{pid}"
        )));
    }

    Ok(result)
}

pub(super) fn target_player_sacrifices(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::TargetPlayerSacrifices { filter, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            // Collect matching permanents on the target player's battlefield.
            let qualifying: Vec<ObjectId> = engine.state.players[pi]
                .battlefield
                .iter()
                .copied()
                .filter(|&oid| object_matches_mass_filter(engine, oid, &filter))
                .collect();
            if qualifying.is_empty() {
                events.push(ev_log(format!(
                    "P{pid} has no valid permanents to sacrifice ({spell_label})."
                )));
            } else {
                let candidate_card_ids: Vec<String> = qualifying
                    .iter()
                    .map(|&oid| {
                        engine
                            .state
                            .objects
                            .get(&oid)
                            .map(|o| o.card_id.clone())
                            .unwrap_or_default()
                    })
                    .collect();
                let candidate_names: Vec<String> = candidate_card_ids
                    .iter()
                    .map(|cid| {
                        engine
                            .registry
                            .get(cid)
                            .map(|d| d.name.clone())
                            .unwrap_or_else(|| cid.clone())
                    })
                    .collect();
                let prompt = format!("P{pid}: choose a permanent to sacrifice ({spell_label}).");
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                        rv1::ResolutionChoiceRequired {
                            deciding_player_id: pid,
                            source_object_id: top.id,
                            prompt_text: prompt.clone(),
                            // Revealed: the battlefield is public so no
                            // hidden-zone redaction is needed.
                            choice_kind: custom::ChoiceKind::Revealed as i32,
                            candidate_object_ids: qualifying.clone(),
                            candidate_card_ids,
                            candidate_names,
                            min: 1,
                            max: 1,
                            ordered: false,
                            unique_names: false,
                            candidate_server_card_ids: Vec::new(),
                            candidate_selectable: Vec::new(),
                            resolution_branches: Vec::new(),
                            mana_cost: String::new(),
                            generic_mana_cost: 0,
                            payment_currently_legal: false,
                            reveal_audience: 0,
                            revealed_zone_owner_player_id: None,
                            candidate_source_zones: Vec::new(),
                            combat_defender_options: Vec::new(),
                            waterbend: false,
                            selection_slots: Vec::new(),
                        },
                    )),
                });
                events.push(ev_log(prompt.clone()));
                engine.state.pending_resolution = Some(PendingResolution {
                    deciding_player: pid,
                    presentation: PendingResolutionPresentation {
                        source_object_id: top.id,
                        candidates: qualifying,
                        min: 1,
                        max: 1,
                        ordered: false,
                        unique_names: false,
                        prompt,
                        choice_kind: custom::ChoiceKind::Revealed,
                    },
                    continuation: ResolutionContinuation::Sacrifice {
                        stack: ParkedStackResolution::new(top.clone()),
                    },
                });
                return Ok(EffectOutcome::Suspended);
            }
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn choose_graveyard_card(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ChooseGraveyardCard {
        filter,
        destination,
        optional,
        from_result,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let controller = cx.controller;
    let Some(index) = cx.engine.state.player_idx(controller) else {
        return Ok(EffectOutcome::Continue);
    };
    let exact_result_generations = from_result.map(|result_filter| {
        cx.previous_effect_result
            .cards
            .iter()
            .filter(|entry| entry.action == result_filter.action)
            .filter(|entry| {
                super::super::history::relative_player_set_contains(
                    &cx.engine.state,
                    result_filter.players,
                    controller,
                    entry.affected_player,
                )
            })
            .filter(|entry| {
                result_filter
                    .card_type
                    .is_none_or(|card_type| entry.matched_card_types.contains(&card_type))
            })
            .map(|entry| (entry.object_id, entry.zone_change_generation))
            .collect::<HashSet<_>>()
    });
    let candidates: Vec<ObjectId> = cx.engine.state.players[index]
        .graveyard
        .iter()
        .copied()
        .filter(|oid| {
            let generation = cx
                .engine
                .state
                .zone_change_generation
                .get(oid)
                .copied()
                .unwrap_or(0);
            exact_result_generations
                .as_ref()
                .is_none_or(|entries| entries.contains(&(*oid, generation)))
                && library_card_matches_filter(
                    &cx.engine.state,
                    cx.engine.registry,
                    *oid,
                    Some(&filter),
                )
        })
        .collect();
    if candidates.is_empty() {
        cx.events.push(ev_log(format!(
            "P{controller} has no matching graveyard card ({}).",
            cx.spell_label
        )));
        return Ok(EffectOutcome::Continue);
    }
    let min = if optional { 0 } else { 1 };
    let prompt = if optional {
        format!("P{controller}: you may choose a matching card from your graveyard.")
    } else {
        format!("P{controller}: choose a matching card from your graveyard.")
    };
    let (candidate_card_ids, candidate_names) = candidate_identities(cx.engine, &candidates);
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: controller,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: custom::ChoiceKind::GraveyardCards as i32,
                candidate_object_ids: candidates.clone(),
                candidate_card_ids,
                min,
                max: 1,
                ordered: false,
                candidate_names,
                candidate_server_card_ids: Vec::new(),
                unique_names: false,
                generic_mana_cost: 0,
                payment_currently_legal: false,
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                candidate_selectable: Vec::new(),
                reveal_audience: 0,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: vec![
                    rv1::ChoiceCandidateSourceZone::Graveyard as i32;
                    candidates.len()
                ],
                combat_defender_options: Vec::new(),
                waterbend: false,
                selection_slots: Vec::new(),
            },
        )),
    });
    cx.events.push(ev_log(prompt.clone()));
    cx.engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: controller,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates: candidates.clone(),
            min,
            max: 1,
            ordered: false,
            prompt,
            choice_kind: custom::ChoiceKind::GraveyardCards,
            unique_names: false,
        },
        continuation: ResolutionContinuation::GraveyardChoice {
            stack: ParkedStackResolution::new(cx.top.clone()),
            destination,
            candidate_generations: candidates
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
                .collect(),
            spell_label: cx.spell_label.to_string(),
        },
    });
    Ok(EffectOutcome::Suspended)
}

pub(super) fn move_graveyard_cards(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::MoveGraveyardCards {
        filter,
        destination,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    use tricerules_cards::primitives::GraveyardDestination;
    let targets: Vec<_> = cx
        .targets
        .iter()
        .copied()
        .filter(|oid| {
            graveyard_target_legal(
                cx.engine,
                &filter,
                *oid,
                cx.controller,
                super::super::targeting::TargetSourceIdentity::for_stack_item(cx.engine, cx.top),
                cx.top.trigger_context,
            )
        })
        .collect();
    if let GraveyardDestination::Battlefield { tapped } = destination {
        let entries = targets
            .into_iter()
            .map(|oid| BattlefieldEntryEvent {
                object_id: oid,
                deciding_player: cx.engine.state.objects[&oid].owner,
                destination_controller: cx.controller,
                battle_protector: None,
                face_index: 0,
                unlock_room_door: None,
                chosen_x: 0,
                cast_cost_receipts: vec![],
                player_life_snapshot: cx.engine.player_life_snapshot(),
                tapped,
                set_types: None,
                entry_counters: BTreeMap::new(),
                applied_effects: vec![],
            })
            .collect();
        return Ok(
            if cx.engine.begin_zone_entry_batch(
                cx.top.clone(),
                entries,
                cx.spell_label,
                cx.events,
            )? {
                EffectOutcome::Suspended
            } else {
                EffectOutcome::Continue
            },
        );
    }
    let engine = &mut *cx.engine;
    let snapshot = engine.snapshot_zone_event();
    let (zone, proto, label) = match destination {
        GraveyardDestination::Hand => (Zone::Hand, rv1::permanent_moved::Destination::Hand, "hand"),
        GraveyardDestination::Exile => (
            Zone::Exile,
            rv1::permanent_moved::Destination::Exile,
            "exile",
        ),
        GraveyardDestination::LibraryTop => (
            Zone::Library,
            rv1::permanent_moved::Destination::Library,
            "the top of its owner's library",
        ),
        GraveyardDestination::LibraryBottom => (
            Zone::Library,
            rv1::permanent_moved::Destination::Library,
            "the bottom of its owner's library",
        ),
        GraveyardDestination::Battlefield { .. } => unreachable!(),
    };
    for tid in targets {
        let owner = engine.state.objects[&tid].owner;
        let name = object_display_name(&engine.state, engine.registry, tid);
        move_object_to_zone(&mut engine.state, engine.registry, tid, zone, None)?;
        if destination == GraveyardDestination::LibraryTop {
            let idx = engine
                .state
                .player_idx(owner)
                .ok_or(EngineError::Illegal("graveyard target owner not found"))?;
            engine.state.players[idx].library.retain(|oid| *oid != tid);
            engine.state.players[idx].library.push_front(tid);
        }
        if destination == GraveyardDestination::Exile {
            cx.effect_result.cards.push(payment::card_result_entry(
                &engine.state,
                engine.registry,
                CardResultAction::Exile,
                owner,
                tid,
            ));
        }
        cx.events.push(ev_log(format!(
            "{} moves {name} from graveyard to {label}.",
            cx.spell_label
        )));
        cx.events
            .push(permanent_moved_event(&engine.state, tid, owner, proto));
    }
    engine.fire_zone_triggers(snapshot, vec![]);
    Ok(EffectOutcome::Continue)
}

pub(super) fn return_triggered_card(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ReturnTriggeredCard {
        reference,
        from,
        tapped,
        controller,
        entry_counters,
        set_types,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let (source_id, event_generation) = match reference {
        TriggeredCardReference::AbilitySource => {
            let Some(source_id) = cx.top.source_permanent_id else {
                return Ok(EffectOutcome::Continue);
            };
            (source_id, cx.top.source_zone_change)
        }
        TriggeredCardReference::TriggerObject => {
            let Some(observed) = cx.top.trigger_context.observed_object else {
                return Ok(EffectOutcome::Continue);
            };
            (observed.object_id, observed.zone_change_generation)
        }
    };
    let current_generation = cx
        .engine
        .state
        .zone_change_generation
        .get(&source_id)
        .copied()
        .unwrap_or(0);
    let Some(object) = cx.engine.state.objects.get(&source_id) else {
        return Ok(EffectOutcome::Continue);
    };
    let origin = match object.zone {
        Zone::Graveyard => tricerules_cards::primitives::EventZone::Graveyard,
        Zone::Exile => tricerules_cards::primitives::EventZone::Exile,
        _ => return Ok(EffectOutcome::Continue),
    };
    if !from.contains(&origin) || current_generation != event_generation.saturating_add(1) {
        return Ok(EffectOutcome::Continue);
    }

    let owner = object.owner;
    let destination_controller = match controller {
        ReturnController::Owner => owner,
        ReturnController::AbilityController => cx.controller,
    };
    let object_label = object_display_name(&cx.engine.state, cx.engine.registry, source_id);
    let entry_counters = entry_counters
        .into_iter()
        .map(|placement| (placement.counter, placement.count))
        .collect();
    match cx.engine.begin_battlefield_entry(
        cx.top.clone(),
        BattlefieldEntryEvent {
            object_id: source_id,
            deciding_player: destination_controller,
            destination_controller,
            battle_protector: None,
            face_index: 0,
            unlock_room_door: None,
            chosen_x: 0,
            cast_cost_receipts: Vec::new(),
            player_life_snapshot: cx.engine.player_life_snapshot(),
            tapped,
            set_types,
            entry_counters,
            applied_effects: Vec::new(),
        },
        BattlefieldEntryCompletion::ResolutionEffect {
            owner,
            spell_label: cx.spell_label.to_string(),
            object_label: object_label.clone(),
        },
        cx.events,
    ) {
        super::super::replacement::BattlefieldEntryProgress::Parked => Ok(EffectOutcome::Suspended),
        super::super::replacement::BattlefieldEntryProgress::Ready(entry) => {
            cx.engine.commit_battlefield_entry(entry, None)?;
            cx.events.push(ev_log(format!(
                "{} returns {object_label} to the battlefield.",
                cx.spell_label
            )));
            cx.events.push(permanent_moved_event(
                &cx.engine.state,
                source_id,
                owner,
                rv1::permanent_moved::Destination::Battlefield,
            ));
            Ok(EffectOutcome::Continue)
        }
    }
}

/// CR 701.18 scry enters the shared private top-library partition state machine. Scry's selected
/// cohort goes to the library bottom; the retained cohort may require a second ordering choice.
pub(super) fn scry(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Scry { count } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let count = cx.engine.resolve_amount(
        &count,
        AmountContext::for_stack_item(cx.top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    if count == 0 {
        return Ok(EffectOutcome::Continue);
    }
    begin_library_partition(cx, count, 0, None, PendingLibraryPartitionKind::Scry)
}

/// CR 701.25 surveil and nonkeyword bounded looks share the same private choice contract. The
/// selected cohort goes to the graveyard; cards retained on top are ordered in a second step when
/// necessary.
pub(super) fn library_partition(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::LibraryPartition {
        count,
        top_min,
        top_max,
        kind,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let pending_kind = match kind {
        LibraryPartitionKind::Surveil => PendingLibraryPartitionKind::Surveil,
        LibraryPartitionKind::Look => PendingLibraryPartitionKind::Look,
    };
    begin_library_partition(cx, count, top_min, top_max, pending_kind)
}

fn begin_library_partition(
    cx: &mut EffectCx<'_>,
    count: u32,
    top_min: u32,
    top_max: Option<u32>,
    kind: PendingLibraryPartitionKind,
) -> Result<EffectOutcome, EngineError> {
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let top = cx.top;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    let Some(idx) = engine.state.player_idx(controller) else {
        return Ok(EffectOutcome::Continue);
    };
    // CR 701.18b: scrying fewer cards than asked for is legal — you look at as many as there are.
    let candidates: Vec<ObjectId> = engine.state.players[idx]
        .library
        .iter()
        .take(count as usize)
        .copied()
        .collect();
    let n = candidates.len() as u32;
    if n == 0 {
        let verb = match kind {
            PendingLibraryPartitionKind::Scry => "scries",
            PendingLibraryPartitionKind::Surveil => "surveils",
            PendingLibraryPartitionKind::Look => "looks at",
        };
        events.push(ev_log(format!(
            "P{controller} {verb} {count} with an empty library ({spell_label})."
        )));
        // CR 701.25d: the surveil event happens after the process is complete even when every
        // action was impossible because the library was empty.
        if kind == PendingLibraryPartitionKind::Surveil {
            engine.fire_triggers(&[GameEvent::Surveilled { player: controller }]);
        }
        return Ok(EffectOutcome::Continue);
    }

    let (candidate_card_ids, candidate_names) = candidate_identities(engine, &candidates);
    let effective_top_min = top_min.min(n);
    let effective_top_max = top_max.unwrap_or(n).min(n);
    let destination_min = n - effective_top_max;
    let destination_max = n - effective_top_min;
    let (prompt, choice_kind, candidate_selectable) = match kind {
        PendingLibraryPartitionKind::Scry => (
            format!(
                "Scry {n}: click any number of cards to put on the bottom of your library — they \
                 go down in the order you click them, so the last one ends up bottom-most. The \
                 rest stay on top."
            ),
            custom::ChoiceKind::LibraryTop,
            Vec::new(),
        ),
        PendingLibraryPartitionKind::Surveil => (
            format!(
                "Surveil {n}: click any number of cards to put into your graveyard — the last one \
                 you click becomes the top card of your graveyard. The rest stay on top of your \
                 library."
            ),
            custom::ChoiceKind::LibraryLook,
            vec![true; n as usize],
        ),
        PendingLibraryPartitionKind::Look => (
            format!(
                "Look at the top {n} cards: click between {destination_min} and {destination_max} \
                 cards to put into your graveyard — the last one you click becomes the top card \
                 of your graveyard. The rest stay on top of your library."
            ),
            custom::ChoiceKind::LibraryLook,
            vec![true; n as usize],
        ),
    };
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: controller,
                source_object_id: top.id,
                prompt_text: prompt.clone(),
                choice_kind: choice_kind as i32,
                candidate_object_ids: candidates.clone(),
                candidate_card_ids,
                candidate_names: candidate_names.clone(),
                min: destination_min,
                max: destination_max,
                ordered: true,
                unique_names: false,
                candidate_server_card_ids: Vec::new(),
                candidate_selectable,
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                generic_mana_cost: 0,
                payment_currently_legal: false,
                reveal_audience: 0,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: Vec::new(),
                combat_defender_options: Vec::new(),
                waterbend: false,
                selection_slots: Vec::new(),
            },
        )),
    });
    let verb = match kind {
        PendingLibraryPartitionKind::Scry => "scries",
        PendingLibraryPartitionKind::Surveil => "surveils",
        PendingLibraryPartitionKind::Look => "looks at",
    };
    // The count is public knowledge; the identities are not.
    events.push(ev_log(format!("P{controller} {verb} {n} ({spell_label}).")));
    events.push(ev_log_private(
        format!("P{controller} looks at {}.", candidate_names.join(", ")),
        controller,
    ));
    engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: controller,
        presentation: PendingResolutionPresentation {
            source_object_id: top.id,
            candidates: candidates.clone(),
            min: destination_min,
            max: destination_max,
            ordered: true,
            unique_names: false,
            prompt,
            choice_kind,
        },
        continuation: ResolutionContinuation::LibraryPartition {
            stack: ParkedStackResolution::new(top.clone()),
            looked_at: candidates,
            stage: PendingLibraryPartitionStage::ChooseDestination,
            kind,
        },
    });
    Ok(EffectOutcome::Suspended)
}

/// CR 701.62: look at the top two cards, manifest one, and put the other into the graveyard.
/// The two-card branch parks behind the existing private library image picker; short libraries
/// complete deterministically without asking a meaningless question.
pub(super) fn manifest_dread(cx: &mut EffectCx<'_>) -> Result<EffectOutcome, EngineError> {
    let engine = &mut *cx.engine;
    let controller = cx.controller;
    let Some(player_idx) = engine.state.player_idx(controller) else {
        return Ok(EffectOutcome::Continue);
    };
    let candidates: Vec<ObjectId> = engine.state.players[player_idx]
        .library
        .iter()
        .take(2)
        .copied()
        .collect();
    if candidates.is_empty() {
        cx.events.push(ev_log(format!(
            "P{controller} manifests dread with an empty library ({}).",
            cx.spell_label
        )));
        return Ok(EffectOutcome::Continue);
    }
    if candidates.len() == 1 {
        let object_id = candidates[0];
        engine
            .state
            .objects
            .get_mut(&object_id)
            .expect("library object")
            .face_down = true;
        match engine.begin_battlefield_entry(
            cx.top.clone(),
            BattlefieldEntryEvent {
                object_id,
                deciding_player: controller,
                destination_controller: controller,
                battle_protector: None,
                face_index: 0,
                unlock_room_door: None,
                chosen_x: 0,
                cast_cost_receipts: Vec::new(),
                player_life_snapshot: engine.player_life_snapshot(),
                tapped: false,
                set_types: None,
                entry_counters: BTreeMap::new(),
                applied_effects: Vec::new(),
            },
            BattlefieldEntryCompletion::ManifestDread {
                owner: controller,
                other_object_id: None,
                chosen_library_position: 0,
            },
            cx.events,
        ) {
            super::super::replacement::BattlefieldEntryProgress::Parked => {
                return Ok(EffectOutcome::Suspended);
            }
            super::super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                engine.commit_battlefield_entry(entry, None)?;
                cx.events.push(permanent_moved_event_with_library_position(
                    &engine.state,
                    object_id,
                    controller,
                    rv1::permanent_moved::Destination::Battlefield,
                    0,
                ));
                cx.events
                    .push(ev_log(format!("P{controller} manifests dread.")));
                return Ok(EffectOutcome::Continue);
            }
        }
    }

    let (candidate_card_ids, candidate_names) = candidate_identities(engine, &candidates);
    let prompt =
        "Choose one of the top two cards to manifest. The other will be put into your graveyard."
            .to_string();
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: controller,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: custom::ChoiceKind::ManifestDread as i32,
                candidate_object_ids: candidates.clone(),
                candidate_card_ids,
                candidate_names: candidate_names.clone(),
                min: 1,
                max: 1,
                ordered: false,
                unique_names: false,
                candidate_server_card_ids: Vec::new(),
                candidate_selectable: Vec::new(),
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                generic_mana_cost: 0,
                payment_currently_legal: false,
                reveal_audience: 0,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: Vec::new(),
                combat_defender_options: Vec::new(),
                waterbend: false,
                selection_slots: Vec::new(),
            },
        )),
    });
    cx.events.push(ev_log(format!(
        "P{controller} looks at the top two cards to manifest dread ({}).",
        cx.spell_label
    )));
    cx.events.push(ev_log_private(
        format!("P{controller} looks at {}.", candidate_names.join(", ")),
        controller,
    ));
    engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: controller,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates: candidates.clone(),
            min: 1,
            max: 1,
            ordered: false,
            unique_names: false,
            prompt,
            choice_kind: custom::ChoiceKind::ManifestDread,
        },
        continuation: ResolutionContinuation::ManifestDread {
            stack: ParkedStackResolution::new(cx.top.clone()),
            looked_at: candidates,
        },
    });
    Ok(EffectOutcome::Suspended)
}

/// Look at a bounded top-of-library window, show every card image privately, and let the
/// controller choose at most one matching card. Selection legality stays engine-authored through
/// `candidate_selectable`; the client never derives it from display/Oracle data.
pub(super) fn look_choose_to_hand(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::LookChooseToHand {
        count,
        filter,
        bottom_order,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let controller = cx.controller;
    let Some(idx) = engine.state.player_idx(controller) else {
        return Ok(EffectOutcome::Continue);
    };
    let looked: Vec<ObjectId> = engine.state.players[idx]
        .library
        .iter()
        .take(count as usize)
        .copied()
        .collect();
    if looked.is_empty() {
        cx.events.push(ev_log(format!(
            "P{controller} looks at an empty library ({}).",
            cx.spell_label
        )));
        return Ok(EffectOutcome::Continue);
    }

    let selectable: Vec<bool> = looked
        .iter()
        .map(|&oid| library_card_matches_filter(&engine.state, engine.registry, oid, Some(&filter)))
        .collect();
    let legal: Vec<ObjectId> = looked
        .iter()
        .copied()
        .zip(&selectable)
        .filter_map(|(oid, selectable)| selectable.then_some(oid))
        .collect();
    let (candidate_card_ids, candidate_names) = candidate_identities(engine, &looked);
    let n = looked.len() as u32;
    let prompt = format!(
        "Look at the top {n} cards. Click up to one matching card image to reveal and put into your hand."
    );
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: controller,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: custom::ChoiceKind::LibraryLook as i32,
                candidate_object_ids: looked.clone(),
                candidate_card_ids,
                candidate_names: candidate_names.clone(),
                min: 0,
                max: 1,
                ordered: false,
                unique_names: false,
                candidate_server_card_ids: Vec::new(),
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                generic_mana_cost: 0,
                payment_currently_legal: false,
                candidate_selectable: selectable,
                reveal_audience: 0,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: Vec::new(),
                combat_defender_options: Vec::new(),
                waterbend: false,
                selection_slots: Vec::new(),
            },
        )),
    });
    cx.events.push(ev_log(format!(
        "P{controller} looks at the top {n} cards of their library ({}).",
        cx.spell_label
    )));
    cx.events.push(ev_log_private(
        format!("P{controller} looks at {}.", candidate_names.join(", ")),
        controller,
    ));
    engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: controller,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates: legal,
            min: 0,
            max: 1,
            ordered: false,
            unique_names: false,
            prompt,
            choice_kind: custom::ChoiceKind::LibraryLook,
        },
        continuation: ResolutionContinuation::LibraryLook {
            stack: ParkedStackResolution::new(cx.top.clone()),
            stage: PendingLibraryLookStage::ChooseToHand {
                looked_at: looked,
                bottom_order,
            },
        },
    });
    Ok(EffectOutcome::Suspended)
}

pub(in crate::engine) fn search_zone_combinations(
    available: &[CardSearchZone],
) -> Vec<Vec<CardSearchZone>> {
    let mut combinations = Vec::new();
    for size in 1..=available.len() {
        for mask in 1usize..(1usize << available.len()) {
            if mask.count_ones() as usize == size {
                combinations.push(
                    available
                        .iter()
                        .enumerate()
                        .filter_map(|(index, zone)| ((mask & (1 << index)) != 0).then_some(*zone))
                        .collect(),
                );
            }
        }
    }
    combinations
}

fn search_zone_label(zone: CardSearchZone) -> &'static str {
    match zone {
        CardSearchZone::Hand => "Hand",
        CardSearchZone::Graveyard => "Graveyard",
        CardSearchZone::Library => "Library",
    }
}

fn search_zone_proto(zone: CardSearchZone) -> i32 {
    match zone {
        CardSearchZone::Hand => rv1::ChoiceCandidateSourceZone::Hand as i32,
        CardSearchZone::Graveyard => rv1::ChoiceCandidateSourceZone::Graveyard as i32,
        CardSearchZone::Library => rv1::ChoiceCandidateSourceZone::Library as i32,
    }
}

pub(in crate::engine) struct ZoneSearchRequest {
    pub count: u32,
    pub filter: Option<ZoneCardFilter>,
    pub slots: Vec<SearchSelectionSlot>,
    pub zones: Vec<CardSearchZone>,
    pub destination: SearchDestination,
    pub conditional_destination: Option<ConditionalSearchDestination>,
    pub shuffle: bool,
    pub reveal: bool,
    pub result_id: Option<tricerules_cards::SearchResultId>,
}

pub(in crate::engine) struct SearchRequest {
    pub count: u32,
    pub filter: Option<ZoneCardFilter>,
    pub slots: Vec<SearchSelectionSlot>,
    pub zones: SearchZoneSelection,
    pub destination: SearchDestination,
    pub conditional_destination: Option<ConditionalSearchDestination>,
    pub shuffle: bool,
    pub reveal: bool,
    pub result_id: Option<tricerules_cards::SearchResultId>,
}

pub(in crate::engine) fn park_zone_search_choice(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    top: &StackItem,
    searcher: PlayerId,
    request: ZoneSearchRequest,
) -> Result<(), EngineError> {
    let ZoneSearchRequest {
        count,
        filter,
        slots,
        zones,
        destination,
        conditional_destination,
        shuffle,
        reveal,
        result_id,
    } = request;
    let idx = engine
        .state
        .player_idx(searcher)
        .ok_or(EngineError::UnknownPlayer(searcher))?;
    let heterogeneous = !slots.is_empty();
    let mut candidates = Vec::new();
    let mut candidate_zones = Vec::new();
    for zone in &zones {
        let cohort: Vec<ObjectId> = match zone {
            CardSearchZone::Hand => engine.state.players[idx].hand.clone(),
            CardSearchZone::Graveyard => engine.state.players[idx].graveyard.clone(),
            CardSearchZone::Library => engine.state.players[idx].library.iter().copied().collect(),
        };
        for oid in cohort.into_iter().filter(|oid| {
            if heterogeneous {
                slots.iter().any(|slot| {
                    library_card_matches_filter(
                        &engine.state,
                        engine.registry,
                        *oid,
                        Some(&slot.filter),
                    )
                })
            } else {
                library_card_matches_filter(&engine.state, engine.registry, *oid, filter.as_ref())
            }
        }) {
            candidates.push(oid);
            candidate_zones.push(search_zone_proto(*zone));
        }
    }
    let public_graveyard_match = candidates
        .iter()
        .zip(&candidate_zones)
        .any(|(_, zone)| *zone == rv1::ChoiceCandidateSourceZone::Graveyard as i32);
    let min = if heterogeneous {
        0
    } else if public_graveyard_match || (filter.is_none() && !candidates.is_empty()) {
        count.min(candidates.len() as u32)
    } else {
        0
    };
    let zone_names = zones
        .iter()
        .map(|zone| search_zone_label(*zone))
        .collect::<Vec<_>>()
        .join(" / ");
    let max = if heterogeneous {
        slots.len() as u32
    } else {
        count
    };
    let prompt = format!("P{searcher}: search {zone_names} for up to {max} matching card(s).");
    let (candidate_card_ids, candidate_names) = candidate_identities(engine, &candidates);
    let multi_zone = zones.len() > 1 || zones.first() != Some(&CardSearchZone::Library);
    let choice_kind = if multi_zone {
        custom::ChoiceKind::ZoneSearch
    } else {
        custom::ChoiceKind::LibrarySearch
    };
    let selection_slots: Vec<rv1::ResolutionSelectionSlot> = slots
        .iter()
        .map(|slot| rv1::ResolutionSelectionSlot {
            label: slot.fallback_label(),
            candidate_indices: candidates
                .iter()
                .enumerate()
                .filter_map(|(index, oid)| {
                    library_card_matches_filter(
                        &engine.state,
                        engine.registry,
                        *oid,
                        Some(&slot.filter),
                    )
                    .then_some(index as u32)
                })
                .collect(),
            presentation: stack_child_presentation_ref(
                engine.registry,
                &top.card_id,
                top.face_index,
                StackPresentationSource::for_stack(
                    engine
                        .state
                        .stack_presentations
                        .get(&top.id)
                        .and_then(|stack| stack.primary.as_ref()),
                    top.ability_text.is_none(),
                ),
                PresentationPath::SearchSlot(&slot.slot_id),
                &slot.presentation,
                slot.fallback_label(),
            ),
        })
        .collect();
    let selection_slot_candidates = selection_slots
        .iter()
        .map(|slot| {
            slot.candidate_indices
                .iter()
                .filter_map(|index| candidates.get(*index as usize).copied())
                .collect()
        })
        .collect();
    let candidate_generations = candidates
        .iter()
        .map(|oid| {
            (
                *oid,
                engine
                    .state
                    .zone_change_generation
                    .get(oid)
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect();
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: searcher,
                source_object_id: top.id,
                prompt_text: prompt.clone(),
                choice_kind: choice_kind as i32,
                candidate_object_ids: candidates.clone(),
                candidate_card_ids,
                candidate_names,
                min,
                max,
                ordered: false,
                unique_names: false,
                candidate_server_card_ids: Vec::new(),
                candidate_selectable: Vec::new(),
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                generic_mana_cost: 0,
                payment_currently_legal: false,
                reveal_audience: 0,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: if multi_zone {
                    candidate_zones
                } else {
                    Vec::new()
                },
                combat_defender_options: Vec::new(),
                waterbend: false,
                selection_slots,
            },
        )),
    });
    events.push(ev_log(prompt.clone()));
    engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: searcher,
        presentation: PendingResolutionPresentation {
            source_object_id: top.id,
            candidates,
            min,
            max,
            ordered: false,
            unique_names: false,
            prompt,
            choice_kind,
        },
        continuation: ResolutionContinuation::SearchLibrary {
            stack: ParkedStackResolution::new(top.clone()),
            searcher,
            zones,
            candidate_generations,
            selection_slot_candidates,
            destination,
            conditional_destination,
            shuffle,
            reveal,
            result_id,
        },
    });
    Ok(())
}

pub(in crate::engine) fn begin_search_request(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    top: &StackItem,
    searcher: PlayerId,
    request: SearchRequest,
) -> Result<(), EngineError> {
    let SearchRequest {
        count,
        filter,
        slots,
        zones,
        destination,
        conditional_destination,
        shuffle,
        reveal,
        result_id,
    } = request;
    match zones {
        SearchZoneSelection::Fixed(zones) => park_zone_search_choice(
            engine,
            events,
            top,
            searcher,
            ZoneSearchRequest {
                count,
                filter,
                slots,
                zones,
                destination,
                conditional_destination,
                shuffle,
                reveal,
                result_id,
            },
        ),
        SearchZoneSelection::PlayerChoice(available_zones) => {
            if result_id.is_some() {
                return Err(EngineError::Illegal(
                    "search result binding requires a fixed library search",
                ));
            }
            let combinations = search_zone_combinations(&available_zones);
            let prompt = format!("P{searcher}: choose which zones to search.");
            let branches = combinations
                .iter()
                .enumerate()
                .map(|(index, zones)| rv1::ResolutionBranchOption {
                    branch_index: index as u32,
                    label: zones
                        .iter()
                        .map(|zone| search_zone_label(*zone))
                        .collect::<Vec<_>>()
                        .join(" + "),
                    cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
                    cost_text: String::new(),
                    selectable: true,
                    search_zones: zones.iter().map(|zone| search_zone_proto(*zone)).collect(),
                    presentation: None,
                })
                .collect();
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                    rv1::ResolutionChoiceRequired {
                        deciding_player_id: searcher,
                        source_object_id: top.id,
                        prompt_text: prompt.clone(),
                        choice_kind: custom::ChoiceKind::ResolutionBranch as i32,
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
                        resolution_branches: branches,
                        mana_cost: String::new(),
                        candidate_selectable: Vec::new(),
                        reveal_audience: 0,
                        revealed_zone_owner_player_id: None,
                        candidate_source_zones: Vec::new(),
                        combat_defender_options: Vec::new(),
                        waterbend: false,
                        selection_slots: Vec::new(),
                    },
                )),
            });
            events.push(ev_log(prompt.clone()));
            engine.state.pending_resolution = Some(PendingResolution {
                deciding_player: searcher,
                presentation: PendingResolutionPresentation {
                    source_object_id: top.id,
                    candidates: Vec::new(),
                    min: 1,
                    max: 1,
                    ordered: false,
                    prompt,
                    choice_kind: custom::ChoiceKind::ResolutionBranch,
                    unique_names: false,
                },
                continuation: ResolutionContinuation::SearchZoneScope {
                    stack: ParkedStackResolution::new(top.clone()),
                    searcher,
                    count,
                    available_zones,
                    filter,
                    destination,
                    conditional_destination,
                    shuffle,
                    reveal,
                },
            });
            Ok(())
        }
    }
}

pub(super) fn search_library(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::SearchLibrary {
        who,
        optional,
        filter,
        count,
        count_by_cast_cost,
        slots,
        zones,
        destination,
        conditional_destination,
        shuffle,
        reveal,
        result_id,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let count = count_by_cast_cost.map_or(count, |conditional| {
        if cx.top.cast_cost_condition_matches(&conditional.condition) {
            conditional.if_selected
        } else {
            conditional.otherwise
        }
    });
    let slots = slots
        .into_iter()
        .filter(|slot| {
            slot.enabled_by_cast_cost
                .as_ref()
                .is_none_or(|condition| cx.top.cast_cost_condition_matches(condition))
        })
        .collect();
    let Some(searcher) = super::player_recipients(cx, who).into_iter().next() else {
        return Ok(EffectOutcome::Continue);
    };
    if optional {
        let prompt = format!("P{searcher}: search your library?");
        cx.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: searcher,
                    source_object_id: cx.top.id,
                    prompt_text: prompt.clone(),
                    choice_kind: custom::ChoiceKind::ResolutionBranch as i32,
                    candidate_object_ids: Vec::new(),
                    candidate_card_ids: Vec::new(),
                    candidate_names: Vec::new(),
                    min: 0,
                    max: 1,
                    ordered: false,
                    unique_names: false,
                    candidate_server_card_ids: Vec::new(),
                    candidate_selectable: Vec::new(),
                    resolution_branches: vec![rv1::ResolutionBranchOption {
                        branch_index: 0,
                        label: "Search".into(),
                        cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
                        cost_text: String::new(),
                        selectable: true,
                        search_zones: Vec::new(),
                        presentation: None,
                    }],
                    mana_cost: String::new(),
                    generic_mana_cost: 0,
                    payment_currently_legal: false,
                    reveal_audience: 0,
                    revealed_zone_owner_player_id: None,
                    candidate_source_zones: Vec::new(),
                    combat_defender_options: Vec::new(),
                    waterbend: false,
                    selection_slots: Vec::new(),
                },
            )),
        });
        cx.events.push(ev_log(prompt.clone()));
        cx.engine.state.pending_resolution = Some(PendingResolution {
            deciding_player: searcher,
            presentation: PendingResolutionPresentation {
                source_object_id: cx.top.id,
                candidates: Vec::new(),
                min: 0,
                max: 1,
                ordered: false,
                unique_names: false,
                prompt,
                choice_kind: custom::ChoiceKind::ResolutionBranch,
            },
            continuation: ResolutionContinuation::OptionalSearch {
                stack: ParkedStackResolution::new(cx.top.clone()),
                searcher,
                count,
                filter,
                slots,
                zones,
                destination,
                conditional_destination,
                shuffle,
                reveal,
            },
        });
        return Ok(EffectOutcome::Suspended);
    }
    begin_search_request(
        cx.engine,
        cx.events,
        cx.top,
        searcher,
        SearchRequest {
            count,
            filter,
            slots,
            zones,
            destination,
            conditional_destination,
            shuffle,
            reveal,
            result_id,
        },
    )?;
    // Resolution is now parked; the "resolves." log is emitted by finish_library_search.
    Ok(EffectOutcome::Suspended)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_placement_appends_to_the_owners_library() {
        let decks = Some(vec![
            vec!["forest".to_string(); 12],
            vec!["forest".to_string(); 12],
        ]);
        let mut engine = GameEngine::new(8906, &[0, 1], 20, decks, true).expect("new game");
        let target = engine.state.players[1]
            .library
            .front()
            .copied()
            .expect("card in library");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            target,
            Zone::Battlefield,
            Some(1),
        )
        .expect("put target on battlefield");

        let mut events = Vec::new();
        move_permanent_to_owners_library(
            &mut engine,
            &mut events,
            target,
            LibraryPlacement::Bottom,
            "test effect",
        )
        .expect("put target on bottom");

        assert_eq!(
            engine.state.players[1].library.back().copied(),
            Some(target)
        );
        assert_eq!(
            engine.state.objects.get(&target).expect("target").zone,
            Zone::Library
        );
    }

    #[test]
    fn second_from_top_clamps_to_empty_and_one_card_libraries() {
        for retained_cards in [0, 1, 5] {
            let decks = Some(vec![
                vec!["forest".to_string(); 12],
                vec!["forest".to_string(); 12],
            ]);
            let mut engine = GameEngine::new(159_012 + retained_cards, &[0, 1], 20, decks, true)
                .expect("new game");
            let target = engine.state.players[1]
                .library
                .pop_front()
                .expect("card in library");
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                target,
                Zone::Battlefield,
                Some(1),
            )
            .expect("put target on battlefield");
            engine.state.players[1]
                .library
                .truncate(retained_cards as usize);

            move_permanent_to_owners_library(
                &mut engine,
                &mut Vec::new(),
                target,
                LibraryPlacement::SecondFromTop,
                "test effect",
            )
            .expect("put target second from top");

            assert_eq!(
                engine.state.players[1]
                    .library
                    .iter()
                    .position(|oid| *oid == target),
                Some(if retained_cards == 0 { 0 } else { 1 })
            );
        }
    }

    #[test]
    fn three_search_zones_produce_all_seven_nonempty_combinations_deterministically() {
        use CardSearchZone::{Graveyard, Hand, Library};
        assert_eq!(
            search_zone_combinations(&[Hand, Graveyard, Library]),
            vec![
                vec![Hand],
                vec![Graveyard],
                vec![Library],
                vec![Hand, Graveyard],
                vec![Hand, Library],
                vec![Graveyard, Library],
                vec![Hand, Graveyard, Library],
            ]
        );
    }
}
