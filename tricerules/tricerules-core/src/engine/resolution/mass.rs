use super::*;

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
    let victims = battlefield_objects_matching(&engine.state, engine.registry, &kind);
    let mut destroyed: Vec<(ObjectId, String, PlayerId)> = Vec::new();
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
        let card_id_t = engine.state.objects.get(&tid).map(|o| o.card_id.clone());
        destroy_permanent(&mut engine.state, tid)?;
        events.push(ev_log(format!("{spell_label} destroys {tgt}")));
        if let Some(owner_id) = owner {
            events.push(permanent_moved_event(
                &engine.state,
                tid,
                owner_id,
                rv1::permanent_moved::Destination::Graveyard,
            ));
        }
        if let (Some(cid), Some(ctrl)) = (card_id_t, owner) {
            destroyed.push((tid, cid, ctrl));
        }
    }
    for (tid, cid, ctrl) in destroyed {
        engine.fire_triggers(
            GameEvent::Dies {
                object_id: tid,
                card_id: cid,
                controller: ctrl,
            },
            events,
        );
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn damage_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DamageAll { amount, kind } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let spell_label = cx.spell_label;

    // CR 119: deal damage to each matching permanent. Marking damage mirrors
    // DamageTarget; lethal-damage destruction is left to state-based actions
    // (CR 704.5g), which run immediately after this spell resolves.
    let affected = battlefield_objects_matching(&engine.state, engine.registry, &kind);
    for tid in &affected {
        let tgt = object_display_name(&engine.state, engine.registry, *tid);
        if let Some(o) = engine.state.objects.get_mut(tid) {
            o.damage += amount;
        }
        events.push(ev_log(format!(
            "{spell_label} deals {amount} damage to {tgt}"
        )));
    }

    Ok(EffectOutcome::Continue)
}
