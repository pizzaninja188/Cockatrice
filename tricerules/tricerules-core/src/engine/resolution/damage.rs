use super::*;
use crate::engine::damage::{DamageEvent, DamageRecipient, DamageSpec};

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
    let source_has_lifelink = cx
        .engine
        .resolving_source_has_keyword(cx.top, Keyword::Lifelink);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let spell_label = cx.spell_label;

    // CR 107.3: `amount` may be the cast-time X (Fireball) or a literal (Bolt).
    let amount = engine.resolve_amount(
        &amount,
        AmountContext::for_stack_item(top, top.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    let Some(&tid) = targets.first() else {
        return Ok(EffectOutcome::Continue);
    };
    let recipient = engine
        .state
        .player_idx(tid as i32)
        .map(|index| DamageRecipient::Player(engine.state.players[index].id))
        .unwrap_or(DamageRecipient::Permanent(tid));
    let damage = vec![DamageSpec {
        event: DamageEvent::noncombat(
            resolving_damage_source_id(top),
            top.controller,
            spell_label,
            recipient,
            amount,
        ),
        source_has_deathtouch,
        source_has_lifelink,
    }];
    let Some(completed) = engine.process_or_park_damage_batch(top, damage, events) else {
        return Ok(EffectOutcome::Suspended);
    };
    engine.commit_completed_damage_batch(&completed, events);

    Ok(EffectOutcome::Continue)
}

pub(super) fn creature_deals_damage_equal_to_power(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CreatureDealsDamageEqualToPower { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(source) = cx
        .targets_by_filter
        .first()
        .and_then(|targets| targets.first())
        .copied()
    else {
        return Ok(EffectOutcome::Continue);
    };
    let Some(recipient) = cx
        .targets_by_filter
        .get(1)
        .and_then(|targets| targets.first())
        .copied()
    else {
        return Ok(EffectOutcome::Continue);
    };
    let Some(characteristics) = cx.engine.characteristics(source) else {
        return Ok(EffectOutcome::Continue);
    };
    if !characteristics.is_creature() {
        return Ok(EffectOutcome::Continue);
    }
    let amount = characteristics.power.unwrap_or(0);
    let controller = characteristics.controller;
    let source_has_deathtouch = cx.engine.effective_has_keyword(source, Keyword::Deathtouch);
    let source_has_lifelink = cx.engine.effective_has_keyword(source, Keyword::Lifelink);
    let damage = vec![DamageSpec {
        event: DamageEvent::noncombat(
            source,
            controller,
            object_display_name(&cx.engine.state, cx.engine.registry, source),
            DamageRecipient::Permanent(recipient),
            amount,
        ),
        source_has_deathtouch,
        source_has_lifelink,
    }];
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
    let source_has_lifelink = cx
        .engine
        .resolving_source_has_keyword(cx.top, Keyword::Lifelink);
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
                .resolve_amount(
                    &amount,
                    AmountContext::for_stack_item(cx.top, controller)
                        .with_previous_effect_result(cx.previous_effect_result),
                )
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
                resolving_damage_source_id(cx.top),
                controller,
                spell_label,
                recipient,
                damage_amount,
            ),
            source_has_deathtouch,
            source_has_lifelink,
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
    let amount = cx.engine.resolve_amount(
        &amount,
        AmountContext::for_stack_item(cx.top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    let source_has_lifelink = cx
        .engine
        .resolving_source_has_keyword(cx.top, Keyword::Lifelink);
    // CR 101.4: APNAP for the multi-player recipients, so the log and the life-loss order are
    // reproducible in a replay.
    let recipients = player_recipients(
        &cx.engine.state,
        cx.controller,
        cx.affected_player,
        trigger_object_controller(cx.engine, cx.top),
        who,
    );
    let damage: Vec<_> = recipients
        .into_iter()
        .map(|player| DamageSpec {
            event: DamageEvent::noncombat(
                resolving_damage_source_id(cx.top),
                cx.controller,
                cx.spell_label,
                DamageRecipient::Player(player),
                amount,
            ),
            source_has_deathtouch: false,
            source_has_lifelink,
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

#[cfg(test)]
mod damage_source_tests {
    use super::*;

    fn item(id: ObjectId, source_permanent_id: Option<ObjectId>) -> StackItem {
        StackItem {
            id,
            controller: 0,
            card_id: "test".into(),
            targets: Vec::new(),
            ability_text: source_permanent_id.map(|_| "ability".into()),
            source_permanent_id,
            source_zone_change: 0,
            source_face_change: 0,
            ability_index: None,
            is_triggered: source_permanent_id.is_some(),
            is_copy: false,
            face_index: 0,
            flashback: false,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            trigger_player: None,
            trigger_object: None,
        }
    }

    #[test]
    fn spells_deal_damage_as_the_stack_card_and_abilities_as_their_physical_source() {
        assert_eq!(resolving_damage_source_id(&item(40, None)), 40);
        assert_eq!(resolving_damage_source_id(&item(90, Some(12))), 12);
    }
}
