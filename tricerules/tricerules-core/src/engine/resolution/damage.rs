use super::*;

/// CR 120.3a: `source_label` deals `amount` damage to `player`, after prevention (CR 615.1).
///
/// The single funnel for non-combat damage dealt to a player. `damage_target`'s player branch and
/// the untargeted [`SpellEffectKind::DamagePlayer`] share it, so a future "whenever a source deals
/// damage to a player" trigger — or infect (CR 120.3b) — hangs off one call site.
///
/// `LifeChanged` is emitted even when prevention reduced the damage to 0. That mirrors what the
/// inlined version did; clients repaint an unchanged total, which is harmless.
pub(super) fn apply_damage_to_player(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    player: PlayerId,
    amount: u32,
    source_label: &str,
) {
    let Some(pi) = engine.state.player_idx(player) else {
        return;
    };
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
            "{source_label} deals {amount} damage to P{player}"
        )));
    }
}

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
        // CR 615.1: consume prevention shield before recording damage. A player target is keyed
        // by its player id widened to an ObjectId, the same convention `TargetRef::object_id`
        // uses, so one call covers both branches below.
        let amount = apply_prevention_shield(
            &mut engine.state.damage_prevention_shields,
            tid,
            amount,
            events,
        );
        if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            apply_damage_to_player(engine, events, pid, amount, spell_label);
        } else {
            let tgt = object_display_name(&engine.state, engine.registry, tid);
            let is_creature = engine
                .characteristics(tid)
                .is_some_and(|value| value.is_creature());
            if let Some(t) = engine.state.objects.get_mut(&tid) {
                if t.zone == Zone::Battlefield && is_creature {
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
    let SpellEffectKind::DamageTargets {
        target: filter,
        amount,
        division,
        ..
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    // CR 608.2b: skip targets that became illegal at resolution. Fireball's even division is
    // calculated only after that filtering; Fire's choose-at-cast allocation remains attached to
    // each target independently.
    let even_damage = if matches!(division, DamageDivision::EvenAtResolution) {
        let legal_count = targets
            .iter()
            .filter(|&&tid| {
                super::targeting::target_filter_legal_at_resolution(
                    engine, &filter, tid, controller,
                )
            })
            .count() as u32;
        if legal_count == 0 {
            0
        } else {
            amount
                .resolve(cx.top.chosen_x)
                .checked_div(legal_count)
                .unwrap_or(0)
        }
    } else {
        0
    };
    for (i, &tid) in targets.iter().enumerate() {
        let damage_amount = if matches!(division, DamageDivision::EvenAtResolution) {
            even_damage
        } else {
            cx.target_damage.get(i).copied().unwrap_or(0)
        };
        if damage_amount == 0 {
            continue;
        }
        if !super::targeting::target_filter_legal_at_resolution(engine, &filter, tid, controller) {
            events.push(ev_log(format!(
                "{spell_label}: target {} is no longer legal, skipping.",
                tid
            )));
            continue;
        }
        let damage_amount = apply_prevention_shield(
            &mut engine.state.damage_prevention_shields,
            tid,
            damage_amount,
            events,
        );
        if damage_amount == 0 {
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
            let is_creature = engine
                .characteristics(tid)
                .is_some_and(|value| value.is_creature());
            if let Some(t) = engine.state.objects.get_mut(&tid) {
                if t.zone == Zone::Battlefield && is_creature {
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

/// CR 120.3a + CR 115.1: untargeted damage to a named player (Sulfuric Vortex, Serendib Efreet).
///
/// Deliberately *not* folded into [`damage_targets`]: that one gates both its branches on a
/// per-target `damage_amount == 0` check and logs unconditionally, so sharing a body would
/// change its behaviour.
pub(super) fn damage_player(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DamagePlayer { amount, who } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let amount = amount.resolve(cx.top.chosen_x);
    // CR 101.4: APNAP for the multi-player recipients, so the log and the life-loss order are
    // reproducible in a replay.
    let recipients: Vec<PlayerId> = match who {
        PlayerRecipient::Controller => vec![cx.controller],
        PlayerRecipient::AffectedPlayer => vec![cx.affected_player],
        PlayerRecipient::EachOpponent => cx
            .engine
            .state
            .players
            .iter()
            .filter(|p| p.id != cx.controller && !p.has_lost)
            .map(|p| p.id)
            .collect(),
        PlayerRecipient::EachPlayer => cx
            .engine
            .state
            .players
            .iter()
            .filter(|p| !p.has_lost)
            .map(|p| p.id)
            .collect(),
    };
    for player in recipients {
        // CR 615.1: each recipient's own prevention shield applies to their share.
        let dealt = apply_prevention_shield(
            &mut cx.engine.state.damage_prevention_shields,
            player as ObjectId,
            amount,
            cx.events,
        );
        apply_damage_to_player(cx.engine, cx.events, player, dealt, cx.spell_label);
    }
    Ok(EffectOutcome::Continue)
}
