use super::*;
use crate::engine::set_tapped;

pub(super) fn destroy_target(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DestroyTarget { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        let indestructible = engine.effective_has_keyword(tid, Keyword::Indestructible);
        if indestructible {
            events.push(ev_log(format!(
                "{spell_label} has no effect: {tgt} is indestructible."
            )));
        } else if consume_regen_shield(&mut engine.state, tid, events) {
            events.push(ev_log(format!("{tgt} regenerates.")));
        } else {
            events.push(ev_log(format!("{spell_label} destroys {tgt}")));
            let owner = engine.state.objects.get(&tid).map(|o| o.owner);
            let controller = engine.state.objects.get(&tid).map(|o| o.controller);
            let card_id_t = engine.state.objects.get(&tid).map(|o| o.card_id.clone());
            let was_creature = engine
                .characteristics(tid)
                .is_some_and(|value| value.is_creature());
            destroy_permanent(&mut engine.state, engine.registry, tid)?;
            if let Some(owner_id) = owner {
                events.push(permanent_moved_event(
                    &engine.state,
                    tid,
                    owner_id,
                    rv1::permanent_moved::Destination::Graveyard,
                ));
            }
            if let (Some(cid), Some(ctrl)) = (card_id_t, controller) {
                engine.fire_triggers(
                    GameEvent::Dies {
                        object_id: tid,
                        card_id: cid,
                        controller: ctrl,
                        was_creature,
                    },
                    events,
                );
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

    if let Some(&tid) = targets.first() {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        let on_battlefield = engine
            .state
            .objects
            .get(&tid)
            .is_some_and(|o| o.zone == Zone::Battlefield);
        if on_battlefield && set_tapped(&mut engine.state, tid, tapped) {
            let verb = if tapped { "taps" } else { "untaps" };
            events.push(ev_log(format!("{spell_label} {verb} {tgt}")));
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn tap_target(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::TapTarget { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    set_target_tapped(cx, true)
}

pub(super) fn untap_target(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::UntapTarget { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    set_target_tapped(cx, false)
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
                            RelativePlayerSet::Opponents => {
                                characteristics.controller != controller
                            }
                            RelativePlayerSet::All => true,
                        }
                })
        })
        .collect();
    let mut tapped = 0;
    for oid in affected {
        let on_battlefield = cx
            .engine
            .state
            .objects
            .get(&oid)
            .is_some_and(|o| o.zone == Zone::Battlefield);
        if on_battlefield && set_tapped(&mut cx.engine.state, oid, true) {
            tapped += 1;
        }
    }
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
                eq.attached_to = Some(target_id);
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
        let shield = engine
            .state
            .damage_prevention_shields
            .entry(tid)
            .or_insert(0);
        *shield = shield.saturating_add(amount);
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
    engine.state.prevent_all_combat_damage_this_turn = true;
    events.push(ev_log(
        "All combat damage is prevented this turn.".to_string(),
    ));

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
    let SpellEffectKind::Regenerate { target } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let spell_label = cx.spell_label;

    let tid = if matches!(target.kind, TargetKind::Self_) {
        top.source_permanent_id
            .filter(|_| engine.source_is_current_object(top))
    } else {
        targets.first().copied()
    };
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
    let targets = cx.targets;
    let top = cx.top;
    let spell_label = cx.spell_label;

    if let (Some(enchanted_oid), Some(obj)) =
        (targets.first().copied(), engine.state.objects.get(&top.id))
    {
        if obj.zone == Zone::Battlefield {
            let tgt = object_display_name(&engine.state, engine.registry, enchanted_oid);
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::AuraAttached(rv1::AuraAttached {
                    aura_object_id: top.id,
                    enchanted_object_id: enchanted_oid,
                })),
            });
            events.push(ev_log(format!("{spell_label} attaches to {tgt}.")));
        }
    }

    Ok(EffectOutcome::Continue)
}
