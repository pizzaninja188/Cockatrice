use super::*;

pub(super) fn damage_target(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DamageTarget { amount, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let spell_label = cx.spell_label;

    // CR 107.3: `amount` may be the cast-time X (Fireball) or a literal (Bolt).
    let amount = amount.resolve(top.chosen_x);
    if let Some(&tid) = targets.first() {
        // CR 614.1a: consume prevention shield before recording damage.
        let amount = apply_prevention_shield(
            &mut engine.state.damage_prevention_shields,
            tid,
            amount,
            events,
        );
        if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            engine.state.players[pi].life -= amount as i32;
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: engine.state.players[pi].id,
                    new_total: engine.state.players[pi].life,
                    delta: -(amount as i32),
                })),
            });
            if amount > 0 {
                events.push(ev_log(format!(
                    "{spell_label} deals {amount} damage to P{pid}"
                )));
            }
        } else {
            let tgt = object_display_name(&engine.state, engine.registry, tid);
            if let Some(t) = engine.state.objects.get_mut(&tid) {
                if t.zone == Zone::Battlefield && t.is_creature(engine.registry) {
                    t.damage += amount;
                    if amount > 0 {
                        events.push(ev_log(format!(
                            "{spell_label} deals {amount} damage to {tgt}"
                        )));
                    }
                }
            }
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn damage_targets(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DamageTargets { target: filter, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    // CR 608.2b: skip targets that became illegal at resolution; apply damage
    // to each remaining legal target using its allocated amount.
    for (i, &tid) in targets.iter().enumerate() {
        let damage_amount = top.target_damage.get(i).copied().unwrap_or(0);
        if damage_amount == 0 {
            continue;
        }
        if !super::targeting::target_filter_legal_at_resolution(
            &engine.state,
            engine.registry,
            &filter,
            tid,
            controller,
        ) {
            events.push(ev_log(format!(
                "{spell_label}: target {} is no longer legal, skipping.",
                tid
            )));
            continue;
        }
        if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            engine.state.players[pi].life -= damage_amount as i32;
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: pid,
                    new_total: engine.state.players[pi].life,
                    delta: -(damage_amount as i32),
                })),
            });
            events.push(ev_log(format!(
                "{spell_label} deals {damage_amount} damage to P{pid}"
            )));
        } else {
            let tgt = object_display_name(&engine.state, engine.registry, tid);
            if let Some(t) = engine.state.objects.get_mut(&tid) {
                if t.zone == Zone::Battlefield && t.is_creature(engine.registry) {
                    t.damage += damage_amount;
                    events.push(ev_log(format!(
                        "{spell_label} deals {damage_amount} damage to {tgt}"
                    )));
                }
            }
        }
    }

    Ok(EffectOutcome::Continue)
}
