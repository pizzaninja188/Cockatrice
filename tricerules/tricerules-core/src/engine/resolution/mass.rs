use super::destruction::{attempt_destroy, DestroyLogStyle, DestroyOutcome, DestroySnapshot};
use super::*;
use crate::engine::{attempt_untap, prepare_and_commit_untaps, UntapOutcome};

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
    let Some(target) = cx.first_current_battlefield_target(0) else {
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
            let characteristics = engine.characteristics(oid);
            let was_creature = characteristics
                .as_ref()
                .is_some_and(|characteristics| characteristics.is_creature());
            DestroySnapshot {
                object_id: oid,
                name,
                indestructible,
                owner,
                source,
                characteristics,
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
    engine.fire_zone_triggers(zone_snapshot, trigger_events, events);

    Ok(EffectOutcome::Continue)
}

pub(super) fn exile_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ExileAll {
        kind,
        max_mana_value_limit,
        until_source_leaves,
    } = effect
    else {
        return Err(EngineError::Illegal("mass exile dispatch mismatch"));
    };
    let dynamic_max_mana_value = match max_mana_value_limit {
        None => None,
        Some(tricerules_card_model::primitives::ManaValueLimit::ActivatedAbilityX) => {
            let valid_loyalty_x_owner = !cx.top.is_triggered
                && cx.top.activated_ability.as_ref().is_some_and(|ability| {
                    ability
                        .costs
                        .iter()
                        .filter(|cost| *cost == &AbilityCost::LoyaltyX)
                        .count()
                        == 1
                });
            if !valid_loyalty_x_owner {
                return Err(EngineError::Illegal(
                    "ExileAll announced-X bound requires an activated LoyaltyX owner",
                ));
            }
            if cx.top.chosen_x > i32::MAX as u32 {
                return Err(EngineError::Illegal(
                    "ExileAll announced-X bound is outside the valid LoyaltyX range",
                ));
            }
            // X was checked and captured when this ability was activated. Keep using the stack
            // value even if paying its loyalty cost caused the source to leave by state-based action.
            Some(cx.top.chosen_x)
        }
        Some(tricerules_card_model::primitives::ManaValueLimit::TriggerObject) => {
            return Err(EngineError::Illegal(
                "ExileAll cannot consume the TriggerObject mana-value bound",
            ));
        }
    };
    let engine = &mut *cx.engine;
    let watched_source = if until_source_leaves {
        let Some(source_id) = cx.top.source_permanent_id else {
            return Ok(EffectOutcome::Continue);
        };
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source_id)
            .copied()
            .unwrap_or(0);
        let Some(source) = engine.state.objects.get(&source_id) else {
            return Ok(EffectOutcome::Continue);
        };
        if source.zone != Zone::Battlefield || source_generation != cx.top.source_zone_change {
            return Ok(EffectOutcome::Continue);
        }
        Some(TriggerObjectRef {
            object_id: source_id,
            zone_change_generation: source_generation,
            controller_at_event: source.controller,
        })
    } else {
        None
    };
    let zone_snapshot = engine.snapshot_zone_event();
    // Freeze every departure before committing any: a departing anthem/type source or
    // attachment must not change another simultaneous victim's LKI (CR 400.7, 603.10). The
    // same snapshot also binds each optional return observer to the generation created here.
    let victims = battlefield_objects_matching(engine, &kind)
        .into_iter()
        .filter(|object_id| {
            dynamic_max_mana_value.is_none_or(|maximum| {
                engine
                    .characteristics(*object_id)
                    .is_some_and(|characteristics| characteristics.mana_value <= maximum)
            })
        })
        .map(|oid| {
            let movement =
                prepare_zone_move(&engine.state, engine.registry, oid, Zone::Exile, None)?
                    .ok_or(EngineError::Illegal("mass exile permanent missing"))?;
            let expected_exiled_generation = movement.prior_generation + 1;
            let object = &engine.state.objects[&oid];
            Ok((
                movement,
                oid,
                object.owner,
                object.controller,
                object_display_name(&engine.state, engine.registry, oid),
                engine.battlefield_leave_event(oid),
                expected_exiled_generation,
            ))
        })
        .collect::<Result<Vec<_>, EngineError>>()?;

    // Install the complete generation-bound return cohort before the first commit. This matters
    // when the linked source is itself in the matching cohort, including an animated source that
    // sorts before the other victims.
    if let Some(watched) = watched_source {
        for (_, object_id, owner, _, _, _, generation) in &victims {
            engine
                .state
                .active_event_observers
                .push(ActiveEventObserver {
                    watched: Some(watched),
                    matcher: EventObserverMatcher::WhenWatchedObjectLeavesBattlefield,
                    payload: EventObserverPayload::ReturnExiledObject {
                        exiled: TriggerObjectRef {
                            object_id: *object_id,
                            zone_change_generation: *generation,
                            controller_at_event: *owner,
                        },
                    },
                });
        }
    }

    let mut leave_events = Vec::with_capacity(victims.len());
    for (movement, oid, owner, controller, name, leave_event, generation) in victims {
        commit_zone_move(&mut engine.state, engine.registry, movement)?;
        debug_assert_eq!(
            engine.state.zone_change_generation.get(&oid).copied(),
            Some(generation)
        );
        cx.effect_result.produced_objects.push(TriggerObjectRef {
            object_id: oid,
            zone_change_generation: generation,
            controller_at_event: controller,
        });
        cx.events
            .push(ev_log(format!("{} exiles {name}", cx.spell_label)));
        cx.events.push(permanent_moved_event(
            &engine.state,
            oid,
            owner,
            rv1::permanent_moved::Destination::Exile,
        ));
        leave_events.extend(leave_event);
    }
    engine.fire_zone_triggers(zone_snapshot, leave_events, cx.events);
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
    // Snapshot every destruction-relevant fact before the first permanent moves. In particular,
    // an earlier Aura or other static-effect source may leave during this simultaneous event;
    // its departure must not change whether a later member of the cohort is indestructible.
    let victims = battlefield_objects_matching(engine, &kind)
        .into_iter()
        .map(|tid| {
            let characteristics = engine.characteristics(tid);
            DestroySnapshot {
                object_id: tid,
                indestructible: engine.effective_has_keyword(tid, Keyword::Indestructible),
                name: object_display_name(&engine.state, engine.registry, tid),
                owner: engine.state.objects.get(&tid).map(|object| object.owner),
                source: engine.trigger_source_snapshot(tid),
                was_creature: characteristics
                    .as_ref()
                    .is_some_and(|value| value.is_creature()),
                characteristics,
            }
        })
        .collect::<Vec<_>>();
    let zone_snapshot = engine.snapshot_zone_event();
    let mut destroyed = Vec::new();
    let mut tap_events = Vec::new();
    for snapshot in victims {
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
    engine.fire_zone_triggers(zone_snapshot, trigger_events, events);

    Ok(EffectOutcome::Continue)
}

/// Shared selection for mass tap and untap (Cryptic Command / Vitalize). CR 115.10 / 608.2h:
/// snapshot current characteristics at resolution without checking targetability. Preserve the
/// selector's deterministic player-then-battlefield order; `players` alone owns controller scope.
fn scoped_battlefield_objects_with_affected_player(
    engine: &GameEngine,
    controller: PlayerId,
    players: MassPlayerSet,
    affected_player: PlayerId,
    filter: &TargetFilter,
    target_receipts: &[StackTarget],
) -> Vec<ObjectId> {
    let targeted_player = match players {
        MassPlayerSet::TargetedPlayer { group_index, .. } => {
            scoped_player_target(engine, target_receipts, group_index)
        }
        _ => None,
    };
    battlefield_objects_matching(engine, filter)
        .into_iter()
        .filter(|oid| {
            engine
                .characteristics(*oid)
                .is_some_and(|characteristics| match players {
                    MassPlayerSet::Controller => characteristics.controller == controller,
                    MassPlayerSet::Opponents => engine
                        .state
                        .are_opponents(characteristics.controller, controller),
                    MassPlayerSet::AffectedPlayer => characteristics.controller == affected_player,
                    MassPlayerSet::All => true,
                    MassPlayerSet::TargetedPlayer { .. } => {
                        targeted_player == Some(characteristics.controller)
                    }
                })
        })
        .collect()
}

#[cfg(test)]
fn scoped_battlefield_objects(
    engine: &GameEngine,
    controller: PlayerId,
    players: impl Into<MassPlayerSet>,
    filter: &TargetFilter,
    target_receipts: &[StackTarget],
) -> Vec<ObjectId> {
    scoped_battlefield_objects_with_affected_player(
        engine,
        controller,
        players.into(),
        controller,
        filter,
        target_receipts,
    )
}

pub(super) fn sacrifice_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::SacrificeAll { players, filter } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let cohort = scoped_battlefield_objects_with_affected_player(
        cx.engine,
        cx.controller,
        players,
        cx.affected_player,
        &filter,
        cx.target_receipts,
    );
    let snapshot = cx.engine.snapshot_zone_event();
    let before = cohort
        .iter()
        .map(|&oid| {
            let source = snapshot
                .source(oid)
                .ok_or(EngineError::Illegal("sacrifice source missing"))?;
            let name = object_display_name(&cx.engine.state, cx.engine.registry, oid);
            let was_creature = source.types.iter().any(|kind| kind == "Creature");
            Ok((oid, source, name, was_creature))
        })
        .collect::<Result<Vec<_>, EngineError>>()?;
    // Freeze every replacement destination before the first member leaves the battlefield.
    let died = sacrifice_permanents(&mut cx.engine.state, cx.engine.registry, &cohort)?;
    let mut departures = Vec::new();
    for ((oid, source, name, was_creature), died) in before.into_iter().zip(died) {
        cx.events
            .push(ev_log(format!("P{} sacrifices {name}.", source.controller)));
        cx.events.push(permanent_moved_event(
            &cx.engine.state,
            oid,
            source.owner,
            rv1::permanent_moved::Destination::Graveyard,
        ));
        cx.effect_result.cards.push(payment::card_result_entry(
            &cx.engine.state,
            cx.engine.registry,
            CardResultAction::Sacrifice,
            source.controller,
            oid,
        ));
        departures.push(super::super::mass_sacrifice::SacrificeDeparture {
            source,
            was_creature,
            died,
        });
    }
    let GameEvent::ZoneChanges(zone) = cx.engine.finish_zone_event(snapshot) else {
        unreachable!()
    };
    if cx
        .engine
        .begin_mass_sacrifice_order(cx.top.clone(), zone, departures, cx.events)
    {
        Ok(EffectOutcome::Suspended)
    } else {
        Ok(EffectOutcome::Continue)
    }
}

pub(super) fn choose_and_sacrifice(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ChooseAndSacrifice {
        players,
        count,
        filter,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let choices = super::player_recipients(cx, players)
        .into_iter()
        .map(|player| crate::state::PendingPlayerSacrificeChoice {
            player,
            candidate_generations: Vec::new(),
            required: 0,
        })
        .collect();
    let mut sacrifice = crate::state::PendingPlayerSetSacrifice {
        choices,
        current: 0,
        accepted: Vec::new(),
        count,
        filter,
    };
    let stack = ParkedStackResolution::new(cx.top.clone());
    if cx
        .engine
        .advance_player_set_sacrifice_choices(&mut sacrifice)
    {
        cx.engine
            .park_player_set_sacrifice_choice(stack, sacrifice, cx.events);
        return Ok(EffectOutcome::Suspended);
    }
    let cohort = sacrifice
        .accepted
        .iter()
        .map(|selection| selection.object_id)
        .collect::<Vec<_>>();
    if cx
        .engine
        .commit_player_set_sacrifice(stack, &cohort, cx.effect_result, cx.events)?
    {
        Ok(EffectOutcome::Suspended)
    } else {
        Ok(EffectOutcome::Continue)
    }
}

pub(super) fn choose_permanents_by_type_and_sacrifice_rest(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ChoosePermanentsByTypeAndSacrificeRest {
        players,
        types,
        scope,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let affected_players = super::player_recipients(cx, players);
    let subjects = affected_players
        .iter()
        .copied()
        .map(|player| crate::state::PendingTypeChoiceSacrificeSubject {
            player,
            departure_cohort: None,
            delegate: None,
            delegate_candidate_slots: Vec::new(),
        })
        .collect();
    let mut sacrifice = crate::state::PendingTypeChoiceSacrifice {
        subjects,
        affected_players,
        current_subject: 0,
        current_type: 0,
        types,
        scope,
        accepted: Vec::new(),
        candidate_generations: Vec::new(),
        stage: crate::state::PendingTypeChoiceSacrificeStage::ChoosingPermanent,
    };
    let stack = ParkedStackResolution::new(cx.top.clone());
    if cx
        .engine
        .advance_type_choice_sacrifice(&stack, &mut sacrifice)?
    {
        cx.engine
            .park_type_choice_sacrifice_choice(stack, sacrifice, cx.events);
        return Ok(EffectOutcome::Suspended);
    }

    let victims = cx.engine.type_choice_sacrifice_victims(&sacrifice);
    if cx
        .engine
        .commit_player_set_sacrifice(stack, &victims, cx.effect_result, cx.events)?
    {
        Ok(EffectOutcome::Suspended)
    } else {
        Ok(EffectOutcome::Continue)
    }
}

pub(super) fn tap_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::TapAll { players, filter } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let affected = scoped_battlefield_objects_with_affected_player(
        cx.engine,
        cx.controller,
        players,
        cx.affected_player,
        &filter,
        cx.target_receipts,
    );
    let tap_events = cx.engine.tap_permanents(cx.controller, &affected);
    let tapped = tap_events.len();
    cx.engine.fire_triggers(&tap_events, cx.events);
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
    let affected = scoped_battlefield_objects_with_affected_player(
        engine,
        cx.controller,
        players,
        cx.affected_player,
        &filter,
        cx.target_receipts,
    );
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

pub(super) fn untap_chosen_permanents(cx: &mut EffectCx<'_>) -> Result<EffectOutcome, EngineError> {
    // CR 608.2f: determine every replacement/prohibition before changing any object.
    // A returned physical object is a new rules object and cannot consume the old receipt.
    let eligible_objects = cx
        .previous_effect_result
        .produced_objects
        .iter()
        .filter(|chosen| {
            cx.engine
                .state
                .objects
                .get(&chosen.object_id)
                .is_some_and(|object| {
                    object.zone == Zone::Battlefield
                        && cx
                            .engine
                            .state
                            .zone_change_generation
                            .get(&chosen.object_id)
                            .copied()
                            .unwrap_or(0)
                            == chosen.zone_change_generation
                })
        })
        .map(|chosen| chosen.object_id)
        .collect::<Vec<_>>();
    let untapped = prepare_and_commit_untaps(cx.engine, &eligible_objects).len();
    cx.events.push(ev_log(format!(
        "{} untaps {untapped} chosen permanent(s)",
        cx.spell_label
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn tap_chosen_permanents(cx: &mut EffectCx<'_>) -> Result<EffectOutcome, EngineError> {
    let Some(ResultType::PermanentSelection {
        chooser,
        filter,
        cardinality,
    }) = cx.previous_effect_result.input_result_type.as_ref()
    else {
        return Err(EngineError::ProgramInvariant(
            "TapChosenPermanents lacks its typed permanent-selection input",
        ));
    };
    let chooser_candidates = super::player_recipients(cx, *chooser);
    let [chooser] = chooser_candidates.as_slice() else {
        return Err(EngineError::Illegal(
            "TapChosenPermanents requires exactly one resolved chooser",
        ));
    };
    let produced_objects = &cx.previous_effect_result.produced_objects;
    if produced_objects.len() < cardinality.min as usize
        || cardinality
            .max
            .is_some_and(|max| produced_objects.len() > max as usize)
    {
        return Err(EngineError::Illegal(
            "chosen permanent selection violates its authored cardinality",
        ));
    }

    let source = TargetSourceIdentity::for_stack_item(cx.engine, cx.top);
    let mut seen = std::collections::HashSet::with_capacity(produced_objects.len());
    for chosen in produced_objects {
        let current_generation = cx
            .engine
            .state
            .zone_change_generation
            .get(&chosen.object_id)
            .copied()
            .unwrap_or(0);
        let Some(object) = cx.engine.state.objects.get(&chosen.object_id) else {
            return Err(EngineError::Illegal(
                "chosen permanent selection contains a stale object",
            ));
        };
        if !seen.insert(chosen.object_id)
            || current_generation != chosen.zone_change_generation
            || object.zone != Zone::Battlefield
            || object.tapped
            || !crate::engine::targeting::permanent_choice_filter_legal(
                cx.engine,
                filter,
                chosen.object_id,
                *chooser,
                source,
                cx.top.trigger_context,
            )
        {
            return Err(EngineError::Illegal(
                "chosen permanent selection is stale or no longer matches its authored filter",
            ));
        }
    }

    let selected = produced_objects
        .iter()
        .map(|chosen| chosen.object_id)
        .collect::<Vec<_>>();
    let tap_events = cx.engine.tap_permanents(cx.controller, &selected);
    let count = tap_events.len().min(u32::MAX as usize) as u32;
    if count > 0 {
        cx.engine.fire_triggers(&tap_events, cx.events);
        cx.events.push(ev_log(format!(
            "{} taps {count} chosen permanent(s)",
            cx.spell_label
        )));
    }
    cx.effect_result.program_value = Some(crate::state::ProgramValue::Count(count));
    Ok(EffectOutcome::Continue)
}

pub(super) fn damage_all(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DamageAll {
        amount,
        players,
        kind,
    } = effect
    else {
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
    let affected = scoped_battlefield_objects_with_affected_player(
        engine,
        cx.controller,
        players,
        cx.affected_player,
        &kind,
        cx.target_receipts,
    );
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
    let Some(completed) = engine.process_or_park_damage_batch(cx.top, damage, events)? else {
        return Ok(EffectOutcome::Suspended);
    };
    engine
        .commit_completed_damage_batch(&completed, events)
        .unwrap();

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
            tricerules_cards::registry::global(),
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
