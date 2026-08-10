use super::*;
use crate::engine::damage::{DamageEvent, DamageRecipient, DamageSpec};

/// The single funnel for non-combat damage dealt to a creature. Prevention is applied before
/// either marked damage or the CR 702.2b deathtouch history bit is recorded.
pub(super) fn apply_damage_to_permanent(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    item: &StackItem,
    event: DamageEvent,
    source_has_deathtouch: bool,
) -> Option<u32> {
    let DamageRecipient::Permanent(target) = event.recipient else {
        return Some(0);
    };
    let result =
        engine.process_or_park_damage_event(item, event.clone(), source_has_deathtouch, events)?;
    let dealt = engine.commit_damage_result(&event, result, source_has_deathtouch, events);
    debug_assert_eq!(event.recipient, DamageRecipient::Permanent(target));
    Some(dealt)
}

pub(super) fn damage_target(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DamageTarget { amount, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let source_has_deathtouch = cx
        .engine
        .resolving_source_has_keyword(cx.top, Keyword::Deathtouch);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let spell_label = cx.spell_label;

    // CR 107.3: `amount` may be the cast-time X (Fireball) or a literal (Bolt).
    let amount = engine.resolve_amount(amount, top.chosen_x);
    if let Some(&tid) = targets.first() {
        if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            let event = DamageEvent::noncombat(
                top.id,
                top.controller,
                spell_label,
                DamageRecipient::Player(pid),
                amount,
            );
            let Some(result) = engine.process_or_park_damage_event(
                top,
                event.clone(),
                source_has_deathtouch,
                events,
            ) else {
                return Ok(EffectOutcome::Suspended);
            };
            engine.commit_damage_result(&event, result, source_has_deathtouch, events);
        } else {
            if apply_damage_to_permanent(
                engine,
                events,
                top,
                DamageEvent::noncombat(
                    top.id,
                    top.controller,
                    spell_label,
                    DamageRecipient::Permanent(tid),
                    amount,
                ),
                source_has_deathtouch,
            )
            .is_none()
            {
                return Ok(EffectOutcome::Suspended);
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
    let source_has_deathtouch = cx
        .engine
        .resolving_source_has_keyword(cx.top, Keyword::Deathtouch);
    let target_source = TargetSourceIdentity::for_stack_item(cx.engine, cx.top);
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
                    engine,
                    &filter,
                    tid,
                    controller,
                    target_source,
                )
            })
            .count() as u32;
        if legal_count == 0 {
            0
        } else {
            engine
                .resolve_amount(amount, cx.top.chosen_x)
                .checked_div(legal_count)
                .unwrap_or(0)
        }
    } else {
        0
    };
    let mut damage = Vec::new();
    for (i, &tid) in targets.iter().enumerate() {
        let damage_amount = if matches!(division, DamageDivision::EvenAtResolution) {
            even_damage
        } else {
            cx.target_damage.get(i).copied().unwrap_or(0)
        };
        if damage_amount == 0 {
            continue;
        }
        if !super::targeting::target_filter_legal_at_resolution(
            engine,
            &filter,
            tid,
            controller,
            target_source,
        ) {
            events.push(ev_log(format!(
                "{spell_label}: target {} is no longer legal, skipping.",
                tid
            )));
            continue;
        }
        let recipient = if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            DamageRecipient::Player(pid)
        } else {
            DamageRecipient::Permanent(tid)
        };
        damage.push(DamageSpec {
            event: DamageEvent::noncombat(
                cx.top.id,
                controller,
                spell_label,
                recipient,
                damage_amount,
            ),
            source_has_deathtouch,
            source_has_lifelink: false,
        });
    }
    let Some(completed) = engine.process_or_park_damage_batch(cx.top, damage, events) else {
        return Ok(EffectOutcome::Suspended);
    };
    engine.commit_completed_damage_batch(&completed, events);

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
    let amount = cx.engine.resolve_amount(amount, cx.top.chosen_x);
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
    let damage: Vec<_> = recipients
        .into_iter()
        .map(|player| DamageSpec {
            event: DamageEvent::noncombat(
                cx.top.id,
                cx.controller,
                cx.spell_label,
                DamageRecipient::Player(player),
                amount,
            ),
            source_has_deathtouch: false,
            source_has_lifelink: false,
        })
        .collect();
    let Some(completed) = cx
        .engine
        .process_or_park_damage_batch(cx.top, damage, cx.events)
    else {
        return Ok(EffectOutcome::Suspended);
    };
    cx.engine
        .commit_completed_damage_batch(&completed, cx.events);
    Ok(EffectOutcome::Continue)
}
