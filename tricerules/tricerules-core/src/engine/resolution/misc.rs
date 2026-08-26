use super::*;
use crate::engine::{attempt_untap, set_tapped, UntapOutcome};

pub(super) fn change_source_face(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ChangeSourceFace { action } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(source_id) = cx.top.source_permanent_id else {
        return Ok(EffectOutcome::Continue);
    };
    let current_face_generation = cx
        .engine
        .state
        .face_change_generation
        .get(&source_id)
        .copied()
        .unwrap_or(0);
    if cx.engine.source_is_current_object(cx.top)
        && current_face_generation == cx.top.source_face_change
    {
        cx.engine
            .change_permanent_face(source_id, action, cx.events)?;
    }
    Ok(EffectOutcome::Continue)
}

pub(super) fn destroy(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Destroy { subject } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let subjects: Vec<ObjectId> = match &subject {
        EffectSubject::Chosen(_) => cx.targets.to_vec(),
        EffectSubject::Source | EffectSubject::AttachedObject | EffectSubject::TriggerObject => {
            resolve_effect_subject(cx.engine, cx.top, cx.targets, &subject)
                .into_iter()
                .collect()
        }
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let spell_label = cx.spell_label;

    for tid in subjects {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        let indestructible = engine.effective_has_keyword(tid, Keyword::Indestructible);
        if indestructible {
            events.push(ev_log(format!(
                "{spell_label} has no effect: {tgt} is indestructible."
            )));
        } else {
            let (regenerated, tap_event) = consume_regen_shield(&mut engine.state, tid, events);
            if regenerated {
                events.push(ev_log(format!("{tgt} regenerates.")));
                engine.fire_triggers(&tap_event.into_iter().collect::<Vec<_>>());
                continue;
            }
            events.push(ev_log(format!("{spell_label} destroys {tgt}")));
            let owner = engine.state.objects.get(&tid).map(|o| o.owner);
            let source = engine.trigger_source_snapshot(tid);
            let was_creature = engine
                .characteristics(tid)
                .is_some_and(|value| value.is_creature());
            let died = destroy_permanent(&mut engine.state, engine.registry, tid)?;
            if let Some(owner_id) = owner {
                events.push(permanent_moved_event(
                    &engine.state,
                    tid,
                    owner_id,
                    rv1::permanent_moved::Destination::Graveyard,
                ));
            }
            if let Some(source) = source {
                engine.fire_triggers(&leaves_and_dies_events(source, was_creature, died));
            }
        }
    }

    Ok(EffectOutcome::Continue)
}

/// CR 701.19 / 701.20: tap or untap the single declared target.
///
/// One body for both directions — they differ only in the flag and the log verb. The target is
/// left alone if it is no longer on the battlefield (CR 608.2b: it changed zones after targeting),
/// and a permanent already in the requested state is a legal target that simply does nothing.
fn set_target_tapped(cx: &mut EffectCx<'_>, tapped: bool) -> Result<EffectOutcome, EngineError> {
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let spell_label = cx.spell_label;

    let mut tap_events = Vec::new();
    for &tid in targets {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        let on_battlefield = engine
            .state
            .objects
            .get(&tid)
            .is_some_and(|o| o.zone == Zone::Battlefield);
        let changed = if !on_battlefield {
            false
        } else if tapped {
            become_tapped(&mut engine.state, tid)
                .map(|event| tap_events.push(event))
                .is_some()
        } else {
            set_tapped(&mut engine.state, tid, false)
        };
        if on_battlefield && changed {
            let verb = if tapped { "taps" } else { "untaps" };
            events.push(ev_log(format!("{spell_label} {verb} {tgt}")));
        }
    }
    engine.fire_triggers(&tap_events);

    Ok(EffectOutcome::Continue)
}

pub(super) fn tap(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Tap { subject } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    if matches!(subject, EffectSubject::Chosen(_)) {
        return set_target_tapped(cx, true);
    }
    let tid = resolve_effect_subject(cx.engine, cx.top, cx.targets, &subject);
    let Some(tid) = tid else {
        return Ok(EffectOutcome::Continue);
    };
    let subject_name = object_display_name(&cx.engine.state, cx.engine.registry, tid);
    let on_battlefield = cx
        .engine
        .state
        .objects
        .get(&tid)
        .is_some_and(|object| object.zone == Zone::Battlefield);
    if on_battlefield {
        let tap_event = become_tapped(&mut cx.engine.state, tid);
        if let Some(event) = tap_event {
            cx.engine.fire_triggers(&[event]);
            cx.events
                .push(ev_log(format!("{} taps {subject_name}", cx.spell_label)));
        }
    }
    Ok(EffectOutcome::Continue)
}

/// CR 502.3 / 611.2a: keep the targeted permanent from untapping during its controller's next
/// untap step. This is rules state rather than tap state: applying it to an already-tapped or
/// already-untapped permanent is meaningful, and the untap-step handler consumes it either way.
pub(super) fn skip_next_untap(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::SkipNextUntap { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    for tid in cx.targets.iter().copied() {
        let on_battlefield = cx
            .engine
            .state
            .objects
            .get(&tid)
            .is_some_and(|object| object.zone == Zone::Battlefield);
        if on_battlefield {
            let generation = cx
                .engine
                .state
                .zone_change_generation
                .get(&tid)
                .copied()
                .unwrap_or(0);
            cx.engine.state.skip_next_untap.insert((tid, generation));
            let subject_name = object_display_name(&cx.engine.state, cx.engine.registry, tid);
            cx.events.push(ev_log(format!(
                "{} keeps {subject_name} from untapping during its controller's next untap step",
                cx.spell_label
            )));
        }
    }
    Ok(EffectOutcome::Continue)
}

pub(super) fn untap(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Untap { subject } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let tid = resolve_effect_subject(cx.engine, cx.top, cx.targets, &subject);
    if let Some(tid) = tid {
        let on_battlefield = cx
            .engine
            .state
            .objects
            .get(&tid)
            .is_some_and(|object| object.zone == Zone::Battlefield);
        if on_battlefield && attempt_untap(&mut cx.engine.state, tid) == UntapOutcome::Untapped {
            let subject_name = object_display_name(&cx.engine.state, cx.engine.registry, tid);
            cx.events
                .push(ev_log(format!("{} untaps {subject_name}", cx.spell_label)));
        }
    }
    Ok(EffectOutcome::Continue)
}

pub(super) fn gain_control_until_end_of_turn(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::GainControlUntilEndOfTurn { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(&target) = cx.targets.first() else {
        return Ok(EffectOutcome::Continue);
    };
    if !cx
        .engine
        .state
        .objects
        .get(&target)
        .is_some_and(|object| object.zone == Zone::Battlefield)
    {
        return Ok(EffectOutcome::Continue);
    }
    cx.engine.state.continuous_effects.push(ContinuousEffect {
        source_id: Some(cx.top.id),
        affected: AffectedScope::Single(target),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(cx.controller),
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: cx.engine.state.command_index,
    });
    cx.engine.reindex_battlefield_control(cx.events);
    cx.events.push(ev_log(format!(
        "{} changes control of {target} until end of turn",
        cx.spell_label
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn create_delayed_trigger(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CreateDelayedTrigger { subject, ability } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let watched_id = match &subject {
        EffectSubject::Source | EffectSubject::AttachedObject | EffectSubject::TriggerObject => {
            resolve_effect_subject(cx.engine, cx.top, cx.targets, &subject)
        }
        EffectSubject::Chosen(target) => cx.targets.first().copied().filter(|object_id| {
            target_filter_legal_at_resolution(
                cx.engine,
                target,
                *object_id,
                cx.controller,
                TargetSourceIdentity::for_stack_item(cx.engine, cx.top),
                cx.top.trigger_context,
            )
        }),
    };
    let Some(watched_id) = watched_id else {
        return Ok(EffectOutcome::Continue);
    };
    let Some(watched_object) = cx.engine.state.objects.get(&watched_id) else {
        return Ok(EffectOutcome::Continue);
    };
    if watched_object.zone != Zone::Battlefield {
        return Ok(EffectOutcome::Continue);
    }
    let watched = TriggerObjectRef {
        object_id: watched_id,
        zone_change_generation: cx
            .engine
            .state
            .zone_change_generation
            .get(&watched_id)
            .copied()
            .unwrap_or(0),
        controller_at_event: watched_object.controller,
    };
    let card_name = cx
        .engine
        .registry
        .get(&cx.top.card_id)
        .map(|definition| definition.name.clone())
        .unwrap_or_else(|| cx.spell_label.to_string());
    let matcher = match ability.trigger {
        TriggerCondition::AtBeginningOfNextEndStep => {
            EventObserverMatcher::AtBeginningOfNextEndStep
        }
        TriggerCondition::AtBeginningOfControllerNextTurnEndStep => {
            EventObserverMatcher::AtBeginningOfControllerNextTurnEndStep {
                controller: cx.controller,
                created_turn_instance: cx.engine.state.turn_instance,
                target_turn_instance: None,
            }
        }
        TriggerCondition::WhenControllerLosesControlOf => {
            EventObserverMatcher::WhenControllerLosesControlOf
        }
        TriggerCondition::WhenWatchedObjectDiesThisTurn => {
            EventObserverMatcher::WhenWatchedObjectDiesThisTurn
        }
        _ => {
            return Err(EngineError::Illegal(
                "delayed trigger has a non-delayed condition",
            ))
        }
    };
    cx.engine
        .state
        .active_event_observers
        .push(ActiveEventObserver {
            watched,
            matcher,
            payload: EventObserverPayload::StageDelayedTrigger(Box::new(DelayedTriggerPayload {
                controller: cx.controller,
                card_id: cx.top.card_id.clone(),
                card_name,
                source_face_index: cx.top.face_index,
                ability: *ability,
            })),
        });
    Ok(EffectOutcome::Continue)
}

pub(super) fn tap_all_creatures(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::TapAllCreatures { players } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let controller = cx.controller;
    let affected: Vec<_> = cx
        .engine
        .state
        .objects
        .keys()
        .copied()
        .filter(|oid| {
            cx.engine
                .characteristics(*oid)
                .is_some_and(|characteristics| {
                    characteristics.is_creature()
                        && match players {
                            RelativePlayerSet::Controller => {
                                characteristics.controller == controller
                            }
                            RelativePlayerSet::Opponents => cx
                                .engine
                                .state
                                .are_opponents(characteristics.controller, controller),
                            RelativePlayerSet::All => true,
                        }
                })
        })
        .collect();
    let mut tapped = 0;
    let mut tap_events = Vec::new();
    for oid in affected {
        let on_battlefield = cx
            .engine
            .state
            .objects
            .get(&oid)
            .is_some_and(|o| o.zone == Zone::Battlefield);
        if on_battlefield {
            if let Some(event) = become_tapped(&mut cx.engine.state, oid) {
                tap_events.push(event);
                tapped += 1;
            }
        }
    }
    cx.engine.fire_triggers(&tap_events);
    cx.events.push(ev_log(format!(
        "{} taps {tapped} affected creature(s)",
        cx.spell_label
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn equip(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Equip { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let spell_label = cx.spell_label;

    let equip_oid = match top.source_permanent_id {
        Some(id) => id,
        None => {
            events.push(ev_log(format!(
                "{spell_label}: equip ability has no source permanent."
            )));
            return Ok(EffectOutcome::Continue);
        }
    };
    if !engine.source_is_current_object(top) {
        events.push(ev_log(format!(
            "{spell_label}: equip source is no longer the same object."
        )));
        return Ok(EffectOutcome::Continue);
    }
    if let Some(&target_id) = targets.first() {
        let valid = engine
            .state
            .objects
            .get(&target_id)
            .is_some_and(|t| t.zone == Zone::Battlefield)
            && engine
                .characteristics(target_id)
                .is_some_and(|value| value.is_creature());
        let equip_on_battlefield = engine
            .state
            .objects
            .get(&equip_oid)
            .map(|e| e.zone == Zone::Battlefield)
            .unwrap_or(false);
        if valid && equip_on_battlefield {
            let tgt = object_display_name(&engine.state, engine.registry, target_id);
            let eq_name = object_display_name(&engine.state, engine.registry, equip_oid);
            if let Some(eq) = engine.state.objects.get_mut(&equip_oid) {
                eq.attached_to = Some(AttachmentRecipient::Object(target_id));
            }
            events.push(ev_log(format!(
                "{spell_label} attaches {eq_name} to {tgt}."
            )));
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn prevent_next_damage(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::PreventNextDamage { amount, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;

    // CR 614.1a: place a damage prevention shield on the target object or player.
    if let Some(&tid) = targets.first() {
        engine.add_damage_prevention(
            Some(cx.top.id),
            cx.spell_label,
            DamagePreventionScope::Recipient(tid),
            DamagePreventionAmount::Remaining(amount),
        );
        let tgt_name = if let Some(pi) = engine.state.player_idx(tid as i32) {
            format!("P{}", engine.state.players[pi].id)
        } else {
            object_display_name(&engine.state, engine.registry, tid)
        };
        events.push(ev_log(format!(
            "Prevention shield: the next {amount} damage to {tgt_name} is prevented."
        )));
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn prevent_all_combat_damage_turn(
    cx: &mut EffectCx<'_>,
    _effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;

    // CR 614.1a: prevent all combat damage this turn (Fog, Holy Day).
    engine.add_damage_prevention(
        Some(cx.top.id),
        cx.spell_label,
        DamagePreventionScope::Combat,
        DamagePreventionAmount::All,
    );
    events.push(ev_log(
        "All combat damage is prevented this turn.".to_string(),
    ));

    Ok(EffectOutcome::Continue)
}

pub(super) fn prevent_all_combat_damage_to_target_turn(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::PreventAllCombatDamageToTargetTurn { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(&target_id) = cx.targets.first() else {
        return Ok(EffectOutcome::Continue);
    };
    let generation = cx
        .engine
        .state
        .zone_change_generation
        .get(&target_id)
        .copied()
        .unwrap_or(0);
    let target_name = object_display_name(&cx.engine.state, cx.engine.registry, target_id);
    cx.engine.add_damage_prevention(
        Some(cx.top.id),
        cx.spell_label,
        DamagePreventionScope::CombatRecipient {
            object_id: target_id,
            zone_change_generation: generation,
        },
        DamagePreventionAmount::All,
    );
    cx.events.push(ev_log(format!(
        "All combat damage that would be dealt to {target_name} this turn is prevented."
    )));

    Ok(EffectOutcome::Continue)
}

pub(super) fn damage_cant_be_prevented_this_turn(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DamageCantBePreventedThisTurn = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    cx.engine
        .state
        .damage_prevention_prohibitions
        .push(DamagePreventionProhibition {
            source_id: Some(cx.top.id),
        });
    cx.events
        .push(ev_log("Damage can't be prevented this turn.".to_string()));
    Ok(EffectOutcome::Continue)
}

pub(super) fn produce_mana(
    _cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ProduceMana { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    Ok(EffectOutcome::Continue)
}

pub(super) fn regenerate(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Regenerate { subject } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let spell_label = cx.spell_label;

    let tid = resolve_effect_subject(engine, top, targets, &subject);
    if let Some(tid) = tid {
        let is_creature = engine
            .state
            .objects
            .get(&tid)
            .is_some_and(|o| o.zone == Zone::Battlefield)
            && engine
                .characteristics(tid)
                .is_some_and(|value| value.is_creature());
        if is_creature {
            let tgt = object_display_name(&engine.state, engine.registry, tid);
            if let Some(o) = engine.state.objects.get_mut(&tid) {
                o.regeneration_shields += 1;
            }
            events.push(ev_log(format!(
                "{tgt} has a regeneration shield ({spell_label})."
            )));
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn none(
    _cx: &mut EffectCx<'_>,
    _effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    Ok(EffectOutcome::Continue)
}

pub(super) fn aura_attach(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::AuraAttach { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let top = cx.top;
    let spell_label = cx.spell_label;

    if let Some(obj) = engine.state.objects.get(&top.id) {
        if obj.zone == Zone::Battlefield {
            let Some(recipient) = obj.attached_to else {
                return Ok(EffectOutcome::Continue);
            };
            let tgt = match recipient {
                AttachmentRecipient::Object(object_id) => {
                    object_display_name(&engine.state, engine.registry, object_id)
                }
                AttachmentRecipient::Player(player_id) => format!("P{player_id}"),
            };
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::AuraAttached(rv1::AuraAttached {
                    aura_object_id: top.id,
                    attachment_recipient: Some(attachment_recipient_proto(recipient)),
                })),
            });
            events.push(ev_log(format!("{spell_label} attaches to {tgt}.")));
        }
    }

    Ok(EffectOutcome::Continue)
}
