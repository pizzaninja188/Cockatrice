use super::destruction::{attempt_destroy, DestroyLogStyle, DestroyOutcome, DestroySnapshot};
use super::*;
use crate::engine::{attempt_untap, UntapOutcome};

#[cfg(test)]
#[path = "mass_tap_tests.rs"]
mod mass_tap_tests;

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
            DestroySnapshot {
                object_id: oid,
                name,
                indestructible,
                owner,
                source,
                was_creature,
            }
        })
        .collect::<Vec<_>>();

    let zone_snapshot = engine.snapshot_zone_event();
    let mut destroyed = Vec::new();
    let mut tap_events = Vec::new();
    for snapshot in snapshots {
        match attempt_destroy(
            engine,
            snapshot,
            false,
            spell_label,
            DestroyLogStyle::Cohort,
            events,
        )? {
            DestroyOutcome::Indestructible => {}
            DestroyOutcome::Regenerated { trigger_events } => tap_events.extend(trigger_events),
            DestroyOutcome::Destroyed { trigger_events, .. } => destroyed.extend(trigger_events),
        }
    }

    let mut trigger_events = tap_events;
    trigger_events.extend(destroyed);
    engine.fire_zone_triggers(zone_snapshot, trigger_events);

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

    // CR 701.8 / 704.4: all matching permanents are destroyed simultaneously,
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
    let zone_snapshot = engine.snapshot_zone_event();
    let mut destroyed = Vec::new();
    let mut tap_events = Vec::new();
    for (tid, source, was_creature) in victims {
        let snapshot = DestroySnapshot {
            object_id: tid,
            indestructible: engine.effective_has_keyword(tid, Keyword::Indestructible),
            name: object_display_name(&engine.state, engine.registry, tid),
            owner: engine.state.objects.get(&tid).map(|object| object.owner),
            source,
            was_creature,
        };
        match attempt_destroy(
            engine,
            snapshot,
            prevent_regeneration,
            spell_label,
            DestroyLogStyle::Cohort,
            events,
        )? {
            DestroyOutcome::Indestructible => {}
            DestroyOutcome::Regenerated { trigger_events } => tap_events.extend(trigger_events),
            DestroyOutcome::Destroyed { trigger_events, .. } => destroyed.extend(trigger_events),
        }
    }
    let mut trigger_events = tap_events;
    trigger_events.extend(destroyed);
    engine.fire_zone_triggers(zone_snapshot, trigger_events);

    Ok(EffectOutcome::Continue)
}

/// Shared selection for mass tap and untap (Cryptic Command / Vitalize). CR 115.10 / 608.2h:
/// snapshot current characteristics at resolution without checking targetability. Preserve the
/// selector's deterministic player-then-battlefield order; `players` alone owns controller scope.
fn scoped_battlefield_objects(
    engine: &GameEngine,
    controller: PlayerId,
    players: RelativePlayerSet,
    filter: &TargetFilter,
) -> Vec<ObjectId> {
    battlefield_objects_matching(engine, filter)
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
        .collect()
}

pub(super) fn tap_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::TapAll { players, filter } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let affected = scoped_battlefield_objects(cx.engine, cx.controller, players, &filter);
    let tap_events = cx.engine.tap_permanents(cx.controller, &affected);
    let tapped = tap_events.len();
    cx.engine.fire_triggers(&tap_events);
    cx.events.push(ev_log(format!(
        "{} taps {tapped} affected permanent(s)",
        cx.spell_label
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn untap_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::UntapAll { players, filter } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let affected = scoped_battlefield_objects(engine, cx.controller, players, &filter);
    let mut untapped = 0;
    for oid in affected {
        if attempt_untap(engine, oid) == UntapOutcome::Untapped {
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
    let amount = cx.engine.resolve_amount(
        &amount,
        AmountContext::for_stack_item(cx.top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
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
