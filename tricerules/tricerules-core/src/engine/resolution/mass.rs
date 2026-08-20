use super::*;
use crate::engine::{attempt_untap, UntapOutcome};

fn attachment_kind_matches(
    characteristics: &super::super::characteristics::Characteristics,
    kind: AttachmentKind,
) -> bool {
    match kind {
        AttachmentKind::Aura => characteristics.is_aura(),
        AttachmentKind::Equipment => characteristics.has_type("Equipment"),
    }
}

fn attached_objects_matching(
    engine: &GameEngine,
    target: ObjectId,
    filter: &AttachmentFilter,
) -> Vec<ObjectId> {
    let mut matches = engine
        .state
        .objects
        .iter()
        .filter_map(|(&oid, object)| {
            (object.zone == Zone::Battlefield
                && object.attached_to == Some(AttachmentRecipient::Object(target))
                && engine.characteristics(oid).is_some_and(|characteristics| {
                    filter
                        .kinds
                        .iter()
                        .any(|&kind| attachment_kind_matches(&characteristics, kind))
                }))
            .then_some(oid)
        })
        .collect::<Vec<_>>();
    matches.sort_unstable();
    matches
}

pub(super) fn destroy_attached(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DestroyAttached { attachments, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(&target) = cx.targets.first() else {
        return Ok(EffectOutcome::Continue);
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let spell_label = cx.spell_label;

    // CR 608.2h: determine this untargeted cohort once, using current derived characteristics.
    // Sorting ObjectIds gives deterministic movement/log order while dies triggers are collected
    // and fired as one logical destruction batch after every successful move.
    let victims = attached_objects_matching(engine, target, &attachments);
    let snapshots = victims
        .into_iter()
        .map(|oid| {
            let name = object_display_name(&engine.state, engine.registry, oid);
            let indestructible = engine.effective_has_keyword(oid, Keyword::Indestructible);
            let owner = engine.state.objects.get(&oid).map(|object| object.owner);
            let source = engine.trigger_source_snapshot(oid);
            let was_creature = engine
                .characteristics(oid)
                .is_some_and(|characteristics| characteristics.is_creature());
            (oid, name, indestructible, owner, source, was_creature)
        })
        .collect::<Vec<_>>();

    let mut destroyed = Vec::new();
    for (oid, name, indestructible, owner, source, was_creature) in snapshots {
        if indestructible {
            events.push(ev_log(format!(
                "{name} is indestructible and survives {spell_label}."
            )));
            continue;
        }
        if consume_regen_shield(&mut engine.state, oid, events) {
            events.push(ev_log(format!("{name} regenerates.")));
            continue;
        }
        destroy_permanent(&mut engine.state, engine.registry, oid)?;
        events.push(ev_log(format!("{spell_label} destroys {name}")));
        if let Some(owner_id) = owner {
            events.push(permanent_moved_event(
                &engine.state,
                oid,
                owner_id,
                rv1::permanent_moved::Destination::Graveyard,
            ));
        }
        if let Some(source) = source {
            destroyed.push((source, was_creature));
        }
    }

    let trigger_events = destroyed
        .into_iter()
        .map(|(source, was_creature)| GameEvent::Dies {
            source,
            was_creature,
        })
        .collect::<Vec<_>>();
    engine.fire_triggers(&trigger_events);

    Ok(EffectOutcome::Continue)
}

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
    let victims = battlefield_objects_matching(engine, &kind)
        .into_iter()
        .map(|tid| {
            let source = engine.trigger_source_snapshot(tid);
            let was_creature = engine
                .characteristics(tid)
                .is_some_and(|value| value.is_creature());
            (tid, source, was_creature)
        })
        .collect::<Vec<_>>();
    let mut destroyed: Vec<(TriggerSourceSnapshot, bool)> = Vec::new();
    for (tid, source, was_creature) in victims {
        let indestructible = engine.effective_has_keyword(tid, Keyword::Indestructible);
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        if indestructible {
            events.push(ev_log(format!(
                "{tgt} is indestructible and survives {spell_label}."
            )));
            continue;
        }
        // CR 701.19c: "can't be regenerated" bypasses shields.
        if !prevent_regeneration && consume_regen_shield(&mut engine.state, tid, events) {
            events.push(ev_log(format!("{tgt} regenerates.")));
            continue;
        }
        let owner = engine.state.objects.get(&tid).map(|o| o.owner);
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
        if let Some(source) = source {
            destroyed.push((source, was_creature));
        }
    }
    let trigger_events: Vec<GameEvent> = destroyed
        .into_iter()
        .map(|(source, was_creature)| GameEvent::Dies {
            source,
            was_creature,
        })
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
    // — the filter's controller relation cannot: untargeted selection has no activating player for
    // `battlefield_objects_matching` to compare against.
    let affected: Vec<_> = battlefield_objects_matching(engine, &filter)
        .into_iter()
        .filter(|oid| {
            engine
                .characteristics(*oid)
                .is_some_and(|characteristics| match players {
                    RelativePlayerSet::Controller => characteristics.controller == controller,
                    RelativePlayerSet::Opponents => engine
                        .state
                        .are_opponents(characteristics.controller, controller),
                    RelativePlayerSet::All => true,
                })
        })
        .collect();
    let mut untapped = 0;
    for oid in affected {
        if attempt_untap(&mut engine.state, oid) == UntapOutcome::Untapped {
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
    let damage: Vec<_> = affected
        .into_iter()
        .map(|tid| crate::engine::damage::DamageSpec {
            event: crate::engine::damage::DamageEvent::noncombat(
                resolving_damage_source_id(cx.top),
                cx.controller,
                spell_label,
                crate::engine::damage::DamageRecipient::Permanent(tid),
                amount,
            ),
            source_has_deathtouch,
            source_has_lifelink: false,
        })
        .collect();
    let Some(completed) = engine.process_or_park_damage_batch(cx.top, damage, events) else {
        return Ok(EffectOutcome::Suspended);
    };
    engine.commit_completed_damage_batch(&completed, events);

    Ok(EffectOutcome::Continue)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn move_to_battlefield(engine: &mut GameEngine, player: usize, card_id: &str) -> ObjectId {
        let oid = if let Some(index) = engine.state.players[player]
            .library
            .iter()
            .position(|oid| engine.state.objects[oid].card_id == card_id)
        {
            engine.state.players[player]
                .library
                .remove(index)
                .expect("known library index")
        } else {
            let index = engine.state.players[player]
                .hand
                .iter()
                .position(|oid| engine.state.objects[oid].card_id == card_id)
                .expect("card in library or hand");
            engine.state.players[player].hand.remove(index)
        };
        engine.state.players[player].battlefield.push(oid);
        engine.state.objects.get_mut(&oid).expect("object").zone = Zone::Battlefield;
        oid
    }

    #[test]
    fn attached_object_filter_handles_aura_equipment_and_combined_cohorts() {
        let mut deck = vec![
            "colossal_dreadmaw".to_string(),
            "holy_strength".to_string(),
            "bonesplitter".to_string(),
            "bottle_gnomes".to_string(),
        ];
        deck.resize(20, "forest".to_string());
        let mut engine = GameEngine::new(
            83_101,
            &[0, 1],
            20,
            Some(vec![deck, vec!["island".to_string(); 20]]),
            true,
        )
        .expect("engine");
        let target = move_to_battlefield(&mut engine, 0, "colossal_dreadmaw");
        let aura = move_to_battlefield(&mut engine, 0, "holy_strength");
        let equipment = move_to_battlefield(&mut engine, 0, "bonesplitter");
        let ordinary_artifact = move_to_battlefield(&mut engine, 0, "bottle_gnomes");
        for oid in [aura, equipment, ordinary_artifact] {
            engine
                .state
                .objects
                .get_mut(&oid)
                .expect("attachment")
                .attached_to = Some(AttachmentRecipient::Object(target));
        }

        assert_eq!(
            attached_objects_matching(
                &engine,
                target,
                &AttachmentFilter {
                    kinds: vec![AttachmentKind::Aura],
                },
            ),
            vec![aura]
        );
        assert_eq!(
            attached_objects_matching(
                &engine,
                target,
                &AttachmentFilter {
                    kinds: vec![AttachmentKind::Equipment],
                },
            ),
            vec![equipment]
        );
        let mut combined = vec![aura, equipment];
        combined.sort_unstable();
        assert_eq!(
            attached_objects_matching(
                &engine,
                target,
                &AttachmentFilter {
                    kinds: vec![AttachmentKind::Aura, AttachmentKind::Equipment],
                },
            ),
            combined
        );
    }
}
