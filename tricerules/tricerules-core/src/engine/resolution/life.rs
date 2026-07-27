use super::*;

pub(super) fn gain_life(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::GainLife { amount } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let top = cx.top;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    let amount = amount.resolve(top.chosen_x);
    let pi = engine.state.player_idx(controller).unwrap();
    engine.state.players[pi].life += amount as i32;
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
            player_id: controller,
            new_total: engine.state.players[pi].life,
            delta: amount as i32,
        })),
    });
    events.push(ev_log(format!(
        "P{controller} gains {amount} life ({spell_label})."
    )));

    Ok(EffectOutcome::Continue)
}

pub(super) fn target_player_gains_life(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::TargetPlayerGainsLife { amount, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            engine.state.players[pi].life += amount as i32;
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: pid,
                    new_total: engine.state.players[pi].life,
                    delta: amount as i32,
                })),
            });
            events.push(ev_log(format!(
                "P{pid} gains {amount} life ({spell_label})."
            )));
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn target_player_loses_life(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::TargetPlayerLosesLife { amount, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            engine.state.players[pi].life -= amount as i32;
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: pid,
                    new_total: engine.state.players[pi].life,
                    delta: -(amount as i32),
                })),
            });
            events.push(ev_log(format!(
                "P{pid} loses {amount} life ({spell_label})."
            )));
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn each_opponent_loses_life_you_gain_equal(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::EachOpponentLosesLifeYouGainEqual { amount } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    let opps: Vec<(usize, PlayerId)> = engine
        .state
        .players
        .iter()
        .enumerate()
        .filter(|(_, p)| p.id != controller && !p.has_lost)
        .map(|(i, p)| (i, p.id))
        .collect();
    let mut total_lost: u32 = 0;
    for (pi, pid) in opps {
        engine.state.players[pi].life -= amount as i32;
        total_lost += amount;
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                player_id: pid,
                new_total: engine.state.players[pi].life,
                delta: -(amount as i32),
            })),
        });
        events.push(ev_log(format!(
            "P{pid} loses {amount} life ({spell_label})."
        )));
    }
    if total_lost > 0 {
        if let Some(ci) = engine.state.player_idx(controller) {
            engine.state.players[ci].life += total_lost as i32;
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: controller,
                    new_total: engine.state.players[ci].life,
                    delta: total_lost as i32,
                })),
            });
            events.push(ev_log(format!(
                "P{controller} gains {total_lost} life ({spell_label})."
            )));
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn drain_target(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DrainTarget { amount, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            engine.state.players[pi].life -= amount as i32;
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: pid,
                    new_total: engine.state.players[pi].life,
                    delta: -(amount as i32),
                })),
            });
            events.push(ev_log(format!(
                "P{pid} loses {amount} life ({spell_label})."
            )));
        }
        if let Some(ci) = engine.state.player_idx(controller) {
            engine.state.players[ci].life += amount as i32;
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: controller,
                    new_total: engine.state.players[ci].life,
                    delta: amount as i32,
                })),
            });
            events.push(ev_log(format!(
                "P{controller} gains {amount} life ({spell_label})."
            )));
        }
    }

    Ok(EffectOutcome::Continue)
}
