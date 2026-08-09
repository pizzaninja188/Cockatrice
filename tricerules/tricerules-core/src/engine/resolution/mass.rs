use super::*;
use crate::engine::set_tapped;

pub(super) fn destroy_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DestroyAll {
        kind,
        prevent_regeneration,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let spell_label = cx.spell_label;

    // CR 701.7 / 704.4: all matching permanents are destroyed simultaneously,
    // then their "dies" triggers fire together. Indestructible permanents survive
    // (CR 702.12b). `prevent_regeneration` bypasses shields (Wrath of God).
    // Untargeted, so hexproof/shroud are irrelevant.
    let victims = battlefield_objects_matching(engine, &kind);
    let mut destroyed: Vec<(ObjectId, String, PlayerId, usize, bool)> = Vec::new();
    for tid in victims {
        let indestructible = engine.effective_has_keyword(tid, Keyword::Indestructible);
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        if indestructible {
            events.push(ev_log(format!(
                "{tgt} is indestructible and survives {spell_label}."
            )));
            continue;
        }
        // CR 701.15b: "can't be regenerated" bypasses shields.
        if !prevent_regeneration && consume_regen_shield(&mut engine.state, tid, events) {
            events.push(ev_log(format!("{tgt} regenerates.")));
            continue;
        }
        let owner = engine.state.objects.get(&tid).map(|o| o.owner);
        let controller = engine.state.objects.get(&tid).map(|o| o.controller);
        let card_id_t = engine.state.objects.get(&tid).map(|o| o.card_id.clone());
        let face_index = engine
            .state
            .objects
            .get(&tid)
            .map(|o| o.face_up_index)
            .unwrap_or(0);
        let was_creature = engine
            .characteristics(tid)
            .is_some_and(|value| value.is_creature());
        destroy_permanent(&mut engine.state, engine.registry, tid)?;
        events.push(ev_log(format!("{spell_label} destroys {tgt}")));
        if let Some(owner_id) = owner {
            events.push(permanent_moved_event(
                &engine.state,
                tid,
                owner_id,
                rv1::permanent_moved::Destination::Graveyard,
            ));
        }
        if let (Some(cid), Some(ctrl)) = (card_id_t, controller) {
            destroyed.push((tid, cid, ctrl, face_index, was_creature));
        }
    }
    let trigger_events: Vec<GameEvent> = destroyed
        .into_iter()
        .map(
            |(object_id, card_id, controller, face_index, was_creature)| GameEvent::Dies {
                source: TriggerSourceSnapshot {
                    object_id,
                    card_id,
                    controller,
                    face_index,
                },
                was_creature,
            },
        )
        .collect();
    engine.fire_triggers(&trigger_events);

    Ok(EffectOutcome::Continue)
}

pub(super) fn untap_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::UntapAll { players, filter } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let controller = cx.controller;
    let engine = &mut *cx.engine;

    // CR 701.20: untargeted, so hexproof/shroud are irrelevant and the battlefield is snapshotted
    // as this resolves. `filter` selects what (creature / any permanent), `players` selects whose
    // — the filter's `only_controller` cannot: untargeted selection has no activating player for
    // `battlefield_objects_matching` to compare against.
    let affected: Vec<_> = battlefield_objects_matching(engine, &filter)
        .into_iter()
        .filter(|oid| {
            engine
                .characteristics(*oid)
                .is_some_and(|characteristics| match players {
                    RelativePlayerSet::Controller => characteristics.controller == controller,
                    RelativePlayerSet::Opponents => characteristics.controller != controller,
                    RelativePlayerSet::All => true,
                })
        })
        .collect();
    let mut untapped = 0;
    for oid in affected {
        if set_tapped(&mut engine.state, oid, false) {
            untapped += 1;
        }
    }
    cx.events.push(ev_log(format!(
        "{} untaps {untapped} affected permanent(s)",
        cx.spell_label
    )));

    Ok(EffectOutcome::Continue)
}

pub(super) fn damage_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DamageAll { amount, kind } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let source_has_deathtouch = cx
        .engine
        .resolving_source_has_keyword(cx.top, Keyword::Deathtouch);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let spell_label = cx.spell_label;

    // CR 119: deal damage to each matching permanent. Marking damage mirrors
    // DamageTarget; lethal-damage destruction is left to state-based actions
    // (CR 704.5g), which run immediately after this spell resolves.
    let affected = battlefield_objects_matching(engine, &kind);
    for tid in &affected {
        super::damage::apply_damage_to_permanent(
            engine,
            events,
            *tid,
            amount,
            source_has_deathtouch,
            spell_label,
        );
    }

    Ok(EffectOutcome::Continue)
}
