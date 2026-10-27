use super::destruction::{attempt_destroy, DestroyLogStyle, DestroyOutcome, DestroySnapshot};
use super::*;

/// CR 716.2a: resolve the level-up ability against its exact source incarnation. The transition
/// event also activates the matching bar ability before the settled public state is published.
pub(super) fn set_class_level(
    cx: &mut EffectCx<'_>,
    level: u32,
) -> Result<EffectOutcome, EngineError> {
    if let Some(source_id) = cx.top.source_permanent_id {
        if let Some(event) =
            cx.engine
                .change_class_level(source_id, cx.top.source_zone_change, level)
        {
            cx.engine.fire_triggers(&[event], cx.events);
        }
    }
    Ok(EffectOutcome::Continue)
}

/// CR 719.3a: the triggered ability marks only the exact Case incarnation that created it.
pub(super) fn solve_case(cx: &mut EffectCx<'_>) -> Result<EffectOutcome, EngineError> {
    let Some(source_id) = cx.top.source_permanent_id else {
        return Ok(EffectOutcome::Continue);
    };
    let generation = cx.top.source_zone_change;
    if cx.engine.source_is_current_object(cx.top)
        && cx
            .engine
            .state
            .objects
            .get(&source_id)
            .is_some_and(|source| source.zone == Zone::Battlefield)
    {
        cx.engine
            .state
            .case_solved_generations
            .insert(source_id, generation);
    }
    Ok(EffectOutcome::Continue)
}
use crate::engine::presentation::{
    stack_child_presentation_ref, PresentationPath, StackPresentationSource,
};
use crate::engine::{attempt_untap, set_tapped, UntapOutcome};

/// Mycosynth Gardens applies a layer-1 copy effect to its exact source incarnation. The target
/// snapshot contains only copiable values; physical identity, counters, attachments, and tapped
/// status remain on the Gardens object.
pub(super) fn copy_target_artifact_with_mana_value_x(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CopyTargetArtifactWithManaValueX { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(target) = cx
        .targets_by_role
        .first()
        .and_then(|targets| targets.first())
        .copied()
    else {
        return Ok(EffectOutcome::Continue);
    };
    let Some(source_id) = cx.top.source_permanent_id else {
        return Ok(EffectOutcome::Continue);
    };
    if !cx.engine.source_is_current_object(cx.top)
        || cx
            .engine
            .state
            .objects
            .get(&source_id)
            .is_none_or(|source| source.zone != Zone::Battlefield)
    {
        return Ok(EffectOutcome::Continue);
    }
    let Some(snapshot) = cx.target_token_copy_snapshot_in_role(target, 0) else {
        return Ok(EffectOutcome::Continue);
    };
    let values = snapshot.values;
    let object = cx
        .engine
        .state
        .objects
        .get_mut(&source_id)
        .expect("the current source object was checked above");
    object.must_attack_if_able = values.face.must_attack_if_able;
    object.must_block_if_able = values.face.must_block_if_able;
    object.copiable_values = Some(values);
    object.copy_revision = object.copy_revision.saturating_add(1);
    object.active_copy_occurrence = Some(object.copy_revision);
    cx.events.push(ev_log(format!(
        "{} becomes a copy of {}.",
        cx.spell_label,
        object_display_name(&cx.engine.state, cx.engine.registry, target),
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn change_source_face(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ChangeSourceFace { action } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(source_id) = cx.top.source_permanent_id else {
        return Ok(EffectOutcome::Continue);
    };
    let current_face_generation = cx
        .engine
        .state
        .face_change_generation
        .get(&source_id)
        .copied()
        .unwrap_or(0);
    if cx.engine.source_is_current_object(cx.top)
        && current_face_generation == cx.top.source_face_change
    {
        cx.engine
            .change_permanent_face(source_id, action, cx.events)?;
    }
    Ok(EffectOutcome::Continue)
}

pub(super) fn destroy(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let (subject, prevent_regeneration) = match effect {
        SpellEffectKind::Destroy { subject } => (subject, false),
        SpellEffectKind::DestroyPreventingRegeneration { subject } => (subject, true),
        _ => return Err(EngineError::Illegal("resolution dispatch mismatch")),
    };
    let subjects: Vec<ObjectId> = match &subject {
        EffectSubject::Chosen(_) => cx.current_battlefield_targets(0),
        EffectSubject::Source
        | EffectSubject::AttachedObject
        | EffectSubject::TriggerObject
        | EffectSubject::PreviousEffectObject => {
            resolve_effect_subject(cx.engine, cx.top, cx.targets, &subject)
                .into_iter()
                .collect()
        }
        EffectSubject::PreviousEffectObjects => {
            return Err(EngineError::Illegal(
                "plural previous-object subject is unsupported for destroy",
            ));
        }
        EffectSubject::TargetAlias(_) => {
            return Err(EngineError::Illegal(
                "unlowered target alias reached destroy resolution",
            ));
        }
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let effect_result = &mut *cx.effect_result;
    let spell_label = cx.spell_label;

    let zone_snapshot = engine.snapshot_zone_event();
    let mut trigger_events = Vec::new();
    for tid in subjects {
        let characteristics = engine.characteristics(tid);
        let snapshot = DestroySnapshot {
            object_id: tid,
            name: object_display_name(&engine.state, engine.registry, tid),
            indestructible: engine.effective_has_keyword(tid, Keyword::Indestructible),
            owner: engine.state.objects.get(&tid).map(|object| object.owner),
            source: zone_snapshot.source(tid),
            was_creature: characteristics
                .as_ref()
                .is_some_and(|value| value.is_creature()),
            characteristics,
        };
        match attempt_destroy(
            engine,
            snapshot,
            prevent_regeneration,
            spell_label,
            DestroyLogStyle::Subject,
            events,
        )? {
            DestroyOutcome::Indestructible => {}
            DestroyOutcome::Regenerated {
                trigger_events: taps,
            } => trigger_events.extend(taps),
            DestroyOutcome::Destroyed {
                receipt,
                trigger_events: departures,
            } => {
                effect_result.cards.extend(receipt);
                trigger_events.extend(departures);
            }
        }
    }

    engine.fire_zone_triggers(zone_snapshot, trigger_events, events);
    Ok(EffectOutcome::Continue)
}

pub(super) fn sacrifice(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Sacrifice { subject } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let subjects: Vec<ObjectId> = match &subject {
        EffectSubject::Chosen(_) => cx.current_battlefield_targets(0),
        EffectSubject::Source
        | EffectSubject::AttachedObject
        | EffectSubject::TriggerObject
        | EffectSubject::PreviousEffectObject => {
            resolve_effect_subject(cx.engine, cx.top, cx.targets, &subject)
                .into_iter()
                .collect()
        }
        EffectSubject::PreviousEffectObjects => {
            return Err(EngineError::Illegal(
                "plural previous-object subject is unsupported for sacrifice",
            ));
        }
        EffectSubject::TargetAlias(_) => {
            return Err(EngineError::Illegal(
                "unlowered target alias reached sacrifice resolution",
            ));
        }
    };
    // CR 701.21a: the instructed player cannot sacrifice a permanent they no longer control.
    // Capture every event's effective characteristics before any member of this instruction moves.
    let sacrifices: Vec<_> = subjects
        .into_iter()
        .filter_map(|oid| {
            let object = cx.engine.state.objects.get(&oid)?;
            if object.zone != Zone::Battlefield {
                return None;
            }
            let source = cx.engine.trigger_source_snapshot(oid)?;
            let characteristics = cx.engine.characteristics(oid)?;
            (source.controller == cx.controller).then(|| {
                (
                    oid,
                    object.owner,
                    object_display_name(&cx.engine.state, cx.engine.registry, oid),
                    source,
                    characteristics,
                )
            })
        })
        .collect();
    let zone_snapshot = cx.engine.snapshot_zone_event();
    let mut committed = Vec::new();
    for (oid, owner, name, source, characteristics) in sacrifices {
        let was_creature = characteristics.is_creature();
        let died = sacrifice_permanent(&mut cx.engine.state, cx.engine.registry, oid)?;
        cx.effect_result.sacrificed_objects.push(TriggerObjectRef {
            object_id: source.object_id,
            zone_change_generation: source.zone_change_generation,
            controller_at_event: source.controller,
        });
        // CR 118.12: a replacement destination still pays this sacrifice instruction.
        let mut receipt = payment::committed_card_result_entry(
            &cx.engine.state,
            cx.engine.registry,
            CardResultAction::Sacrifice,
            cx.controller,
            oid,
            Zone::Battlefield,
            source.zone_change_generation,
        )?;
        receipt.matched_card_types = CardTypeFilter::ALL
            .into_iter()
            .filter(|card_type| {
                payment::card_type_filter_matches_characteristics(*card_type, &characteristics)
            })
            .collect();
        cx.effect_result.cards.push(receipt);
        cx.events.push(permanent_moved_event(
            &cx.engine.state,
            oid,
            owner,
            rv1::permanent_moved::Destination::Graveyard,
        ));
        cx.events
            .push(ev_log(format!("{} sacrifices {name}", cx.spell_label)));
        committed.extend(sacrifice_events(source, was_creature, cx.controller, died));
    }
    cx.engine
        .fire_zone_triggers(zone_snapshot, committed, cx.events);
    Ok(EffectOutcome::Continue)
}

/// CR 701.19 / 701.20: tap or untap the single declared target.
///
/// One body for both directions — they differ only in the flag and the log verb. The target is
/// left alone if it is no longer on the battlefield (CR 608.2b: it changed zones after targeting),
/// and a permanent already in the requested state is a legal target that simply does nothing.
fn set_target_tapped(cx: &mut EffectCx<'_>, tapped: bool) -> Result<EffectOutcome, EngineError> {
    let targets = cx.current_battlefield_targets(0);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let spell_label = cx.spell_label;

    let tap_events = if tapped {
        engine.tap_permanents(cx.controller, &targets)
    } else {
        Vec::new()
    };
    for &tid in &targets {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        let on_battlefield = engine
            .state
            .objects
            .get(&tid)
            .is_some_and(|o| o.zone == Zone::Battlefield);
        let changed = if !on_battlefield {
            false
        } else if tapped {
            tap_events.iter().any(|event| {
                matches!(event,
                GameEvent::BecameTapped { object, .. } if object.object_id == tid)
            })
        } else {
            set_tapped(&mut engine.state, tid, false)
        };
        if on_battlefield && changed {
            let verb = if tapped { "taps" } else { "untaps" };
            events.push(ev_log(format!("{spell_label} {verb} {tgt}")));
        }
    }
    engine.fire_triggers(&tap_events, events);

    Ok(EffectOutcome::Continue)
}

pub(super) fn tap(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Tap { subject } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    if matches!(subject, EffectSubject::Chosen(_)) {
        return set_target_tapped(cx, true);
    }
    let tid = resolve_effect_subject(cx.engine, cx.top, cx.targets, &subject);
    let Some(tid) = tid else {
        return Ok(EffectOutcome::Continue);
    };
    let subject_name = object_display_name(&cx.engine.state, cx.engine.registry, tid);
    let on_battlefield = cx
        .engine
        .state
        .objects
        .get(&tid)
        .is_some_and(|object| object.zone == Zone::Battlefield);
    if on_battlefield {
        let tap_events = cx.engine.tap_permanents(cx.controller, &[tid]);
        if !tap_events.is_empty() {
            cx.engine.fire_triggers(&tap_events, cx.events);
            cx.events
                .push(ev_log(format!("{} taps {subject_name}", cx.spell_label)));
        }
    }
    Ok(EffectOutcome::Continue)
}

/// CR 502.3 / 611.2a: keep the targeted permanent from untapping during its controller's next
/// untap step. This is rules state rather than tap state: applying it to an already-tapped or
/// already-untapped permanent is meaningful, and the untap-step handler consumes it either way.
pub(super) fn skip_next_untap(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::SkipNextUntap { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    for tid in cx.current_battlefield_targets(0) {
        let generation = cx
            .engine
            .state
            .zone_change_generation
            .get(&tid)
            .copied()
            .unwrap_or(0);
        cx.engine.state.skip_next_untap.insert((tid, generation));
        let subject_name = object_display_name(&cx.engine.state, cx.engine.registry, tid);
        cx.events.push(ev_log(format!(
            "{} keeps {subject_name} from untapping during its controller's next untap step",
            cx.spell_label
        )));
    }
    Ok(EffectOutcome::Continue)
}

pub(super) fn untap(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Untap { subject } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    for tid in cx.resolve_battlefield_subjects(&subject) {
        let on_battlefield = cx
            .engine
            .state
            .objects
            .get(&tid)
            .is_some_and(|object| object.zone == Zone::Battlefield);
        if on_battlefield && attempt_untap(cx.engine, tid) == UntapOutcome::Untapped {
            let subject_name = object_display_name(&cx.engine.state, cx.engine.registry, tid);
            cx.events
                .push(ev_log(format!("{} untaps {subject_name}", cx.spell_label)));
        }
    }
    Ok(EffectOutcome::Continue)
}

pub(super) fn set_prepared(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::SetPrepared { subject, prepared } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    if let Some(id) = cx.resolve_battlefield_subject(&subject) {
        if prepared {
            cx.engine.prepare_permanent(id);
        } else {
            crate::engine::preparation::unprepare_permanent(&mut cx.engine.state, id);
        }
    }
    Ok(EffectOutcome::Continue)
}

pub(super) fn gain_control(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::GainControl { duration, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(target) = cx.first_current_battlefield_target(0) else {
        return Ok(EffectOutcome::Continue);
    };
    let duration = match duration {
        GainControlDuration::UntilEndOfTurn => EffectDuration::UntilEndOfTurn,
        GainControlDuration::WhileYouControlSource => {
            let Some(source_object_id) = cx.top.source_permanent_id else {
                return Ok(EffectOutcome::Continue);
            };
            let source_is_current = cx
                .engine
                .state
                .objects
                .get(&source_object_id)
                .is_some_and(|object| object.zone == Zone::Battlefield)
                && cx
                    .engine
                    .state
                    .zone_change_generation
                    .get(&source_object_id)
                    .copied()
                    .unwrap_or(0)
                    == cx.top.source_zone_change;
            let source_controller = cx
                .engine
                .characteristics(source_object_id)
                .map(|characteristics| characteristics.controller);
            if !source_is_current || source_controller != Some(cx.controller) {
                return Ok(EffectOutcome::Continue);
            }
            EffectDuration::WhileSourceControlledBy {
                source_object_id,
                source_zone_change_generation: cx.top.source_zone_change,
                controller: cx.controller,
            }
        }
        GainControlDuration::UntilEndOfControllerNextTurnIfBattlefieldMaximumGreaterThanTargetManaValue {
            filter,
        } => {
            let target_mana_value = cx
                .engine
                .characteristics(target)
                .map_or(0, |characteristics| characteristics.mana_value);
            let maximum = cx.engine.resolve_amount(
                &Amount::Count(CountExpression::BattlefieldMaximum {
                    filter,
                    characteristic: BattlefieldQuantityCharacteristic::ManaValue,
                }),
                AmountContext::for_stack_item(cx.top, cx.controller)
                    .with_previous_effect_result(cx.previous_effect_result),
            );
            if maximum > target_mana_value {
                EffectDuration::UntilEndOfNextTurn {
                    player: cx.controller,
                    created_turn_instance: cx.engine.state.turn_instance,
                }
            } else {
                EffectDuration::UntilEndOfTurn
            }
        }
    };
    let duration_label = match duration {
        EffectDuration::UntilEndOfNextTurn { .. } => "until end of its controller's next turn",
        EffectDuration::WhileSourceControlledBy { .. } => "for as long as you control its source",
        _ => "until end of turn",
    };
    cx.engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: Some(cx.top.id),
        affected: AffectedScope::Single(target),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(cx.controller),
        },
        condition: None,
        duration,
        timestamp: cx.engine.state.command_index,
    });
    cx.engine.reindex_battlefield_control(cx.events);
    cx.events.push(ev_log(format!(
        "{} changes control of {} {duration_label}",
        cx.spell_label,
        object_display_name(&cx.engine.state, cx.engine.registry, target),
    )));
    Ok(EffectOutcome::Continue)
}

/// CR 611.2a / 613.1b: Coveted Jewel's trigger changes control of its exact source permanent to
/// the event-time attacking player. The effect is untargeted and remains until the artifact leaves.
pub(super) fn give_control_of_source_to_attacking_player(
    cx: &mut EffectCx<'_>,
) -> Result<EffectOutcome, EngineError> {
    let Some(source_id) = cx.top.source_permanent_id else {
        return Ok(EffectOutcome::Continue);
    };
    if !cx.engine.source_is_current_object(cx.top) {
        return Ok(EffectOutcome::Continue);
    }
    let Some(attacking_player) = player_recipients(
        cx,
        tricerules_card_model::primitives::PlayerRecipient::TriggeringAttackingPlayer,
    )
    .first()
    .copied() else {
        return Ok(EffectOutcome::Continue);
    };

    cx.engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: Some(source_id),
        affected: AffectedScope::Single(source_id),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(attacking_player),
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: cx.engine.state.command_index,
    });
    cx.engine.reindex_battlefield_control(cx.events);
    cx.events.push(ev_log(format!(
        "P{attacking_player} gains control of {}.",
        object_display_name(&cx.engine.state, cx.engine.registry, source_id)
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn create_delayed_trigger(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CreateDelayedTrigger {
        subject,
        affected_player,
        ability,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let memory_jar_delayed = matches!(
        ability.effect.as_slice(),
        [
            SpellEffectKind::Discard {
                who: PlayerRecipient::EachPlayer,
                quantity: tricerules_card_model::primitives::DiscardQuantity::All
            },
            SpellEffectKind::ReturnMemoryJarHands
        ]
    ) && ability.trigger == TriggerCondition::AtBeginningOfNextEndStep;
    let memory_jar_cohort =
        if memory_jar_delayed {
            let key = cx.top.id;
            if cx
                .engine
                .state
                .player_idx(cx.controller)
                .is_none_or(|index| cx.engine.state.players[index].has_lost)
            {
                cx.engine.state.memory_jar_cohorts.remove(&key);
                return Ok(EffectOutcome::Continue);
            }
            let cohort = cx.engine.state.memory_jar_cohorts.get(&key).ok_or(
                EngineError::ProgramInvariant("Memory Jar delayed trigger lost its hand cohort"),
            )?;
            if cohort.controller != cx.controller {
                return Err(EngineError::ProgramInvariant(
                    "Memory Jar delayed trigger has the wrong controller",
                ));
            }
            Some(key)
        } else {
            None
        };
    let affected_player = match affected_player {
        Some(PlayerRecipient::ProgramInputPlayer) => {
            let Some(crate::state::ProgramValue::TargetedSpellController(Some(player))) =
                cx.previous_effect_result.program_value.as_ref()
            else {
                // A missing typed capture must not silently bind the delayed ability to its
                // controller. A resolving counter effect publishes this only for a legal target.
                return Ok(EffectOutcome::Continue);
            };
            Some(*player)
        }
        Some(_) => {
            return Err(EngineError::Illegal(
                "delayed trigger affected_player requires a single captured player",
            ));
        }
        None => None,
    };
    let exact_previous = if matches!(subject, Some(EffectSubject::PreviousEffectObject)) {
        match cx.previous_effect_result.produced_objects.as_slice() {
            [] => None,
            [object] => Some(*object),
            _ => {
                return Err(EngineError::Illegal(
                    "CreateDelayedTrigger PreviousEffectObject requires a single exact object",
                ));
            }
        }
    } else {
        None
    };
    let token_copy_cohort = if matches!(subject, Some(EffectSubject::PreviousEffectObjects)) {
        if cx.previous_effect_result.produced_object_provenance
            != Some(crate::state::ProducedObjectProvenance::TokenCreationBatch)
        {
            return Err(EngineError::Illegal(
                "CreateDelayedTrigger PreviousEffectObjects requires a token-copy batch receipt",
            ));
        }
        let cohort = cx.previous_effect_result.produced_objects.clone();
        if cohort.is_empty() {
            return Ok(EffectOutcome::Continue);
        }
        Some(cohort)
    } else {
        None
    };
    let emblem_object = matches!(subject, Some(EffectSubject::TriggerObject))
        .then_some(cx.top.trigger_context.emblem_graveyard_object)
        .flatten();
    let watched = if let Some(object) = emblem_object {
        Some(object)
    } else if let Some(subject) = subject.as_ref() {
        let watched_id = match subject {
            EffectSubject::Source
            | EffectSubject::AttachedObject
            | EffectSubject::TriggerObject => {
                resolve_effect_subject(cx.engine, cx.top, cx.targets, subject)
            }
            EffectSubject::PreviousEffectObject => exact_previous.map(|object| object.object_id),
            EffectSubject::PreviousEffectObjects => token_copy_cohort
                .as_ref()
                .and_then(|cohort| cohort.first())
                .map(|object| object.object_id),
            EffectSubject::Chosen(_) => cx.first_current_battlefield_target(0),
            EffectSubject::TargetAlias(_) => {
                return Err(EngineError::Illegal(
                    "unlowered target alias reached delayed-trigger resolution",
                ));
            }
        };
        let Some(watched_id) = watched_id else {
            return Ok(EffectOutcome::Continue);
        };
        let watched = if let Some(previous) = exact_previous {
            previous
        } else if let Some(primary) = token_copy_cohort
            .as_ref()
            .and_then(|cohort| cohort.first())
            .copied()
        {
            primary
        } else {
            let Some(watched_object) = cx.engine.state.objects.get(&watched_id) else {
                return Ok(EffectOutcome::Continue);
            };
            if watched_object.zone != Zone::Battlefield {
                return Ok(EffectOutcome::Continue);
            }
            TriggerObjectRef {
                object_id: watched_id,
                zone_change_generation: cx
                    .engine
                    .state
                    .zone_change_generation
                    .get(&watched_id)
                    .copied()
                    .unwrap_or(0),
                controller_at_event: watched_object.controller,
            }
        };
        Some(watched)
    } else if memory_jar_delayed
        || matches!(
            ability.trigger,
            TriggerCondition::AtBeginningOfNextTurnUpkeep
        )
    {
        None
    } else {
        return Err(EngineError::Illegal(
            "subjectless delayed trigger requires a temporal trigger condition",
        ));
    };
    let card_name = cx.spell_label.to_owned();
    let program_definition = if ability.program.is_some() {
        let mut definition = match cx.top.ability_program_owner() {
            Some(tricerules_card_model::ProgramOwnerId::Ability(definition)) => definition,
            Some(_) => {
                return Err(EngineError::ProgramInvariant(
                    "authored delayed child has no immutable parent ability definition",
                ));
            }
            None if is_spell_stack_item_for_delayed_child(cx.top) => {
                let face = cx
                    .engine
                    .registry
                    .get(&cx.top.card_id)
                    .and_then(|card| card.face(cx.top.face_index))
                    .ok_or(EngineError::ProgramInvariant(
                        "authored delayed spell child has no exact face definition",
                    ))?;
                tricerules_card_model::AbilityDefinitionId {
                    card_id: cx.top.card_id.clone(),
                    face_id: face.face_id.clone(),
                    ability_path: Vec::new(),
                }
            }
            None => {
                return Err(EngineError::ProgramInvariant(
                    "authored delayed child has no immutable parent ability definition",
                ));
            }
        };
        definition.ability_path.push(ability.ability_id.clone());
        let owner = tricerules_card_model::ProgramOwnerId::ability(definition.clone());
        let compiled = cx
            .engine
            .registry
            .compiled_program_for_owner(&owner)
            .ok_or(EngineError::ProgramInvariant(
                "authored delayed child has no exact compiled Program owner",
            ))?;
        if compiled.owner != owner {
            return Err(EngineError::ProgramInvariant(
                "authored delayed child compiled owner does not match its exact definition",
            ));
        }
        Some(definition)
    } else {
        None
    };
    let matcher = match ability.trigger {
        TriggerCondition::AtBeginningOfNextEndStep => {
            EventObserverMatcher::AtBeginningOfNextEndStep
        }
        TriggerCondition::AtBeginningOfControllerNextTurnEndStep => {
            EventObserverMatcher::AtBeginningOfControllerNextTurnEndStep {
                controller: cx.controller,
                created_turn_instance: cx.engine.state.turn_instance,
                target_turn_instance: None,
            }
        }
        TriggerCondition::AtBeginningOfNextTurnUpkeep => {
            EventObserverMatcher::AtBeginningOfNextTurnUpkeep {
                created_turn_instance: cx.engine.state.turn_instance,
                target_turn_instance: None,
            }
        }
        TriggerCondition::WhenControllerLosesControlOf => {
            EventObserverMatcher::WhenControllerLosesControlOf
        }
        TriggerCondition::WhenWatchedObjectDiesThisTurn => {
            EventObserverMatcher::WhenWatchedObjectDiesThisTurn
        }
        TriggerCondition::WhenWatchedObjectDiesOrIsExiled => {
            EventObserverMatcher::WhenWatchedObjectDiesOrIsExiled
        }
        _ => {
            return Err(EngineError::Illegal(
                "delayed trigger has a non-delayed condition",
            ));
        }
    };
    let ability_text = ability.fallback_text(&card_name);
    let parent = cx
        .engine
        .state
        .stack_presentations
        .get(&cx.top.id)
        .and_then(|stack| stack.primary.as_ref());
    let presentation = stack_child_presentation_ref(
        cx.engine.registry,
        &cx.top.card_id,
        cx.top.face_index,
        StackPresentationSource::for_stack(parent, cx.top.ability_text.is_none()),
        PresentationPath::Ability(&ability.ability_id),
        &ability.presentation,
        ability_text,
    );
    if let Some(cohort) = token_copy_cohort {
        let primary = watched.ok_or(EngineError::Illegal("delayed token cohort missing"))?;
        let key = (primary.object_id, primary.zone_change_generation);
        if cx.engine.state.observed_object_cohorts.contains_key(&key) {
            return Err(EngineError::Illegal("delayed token cohort already exists"));
        }
        cx.engine.state.observed_object_cohorts.insert(key, cohort);
    }
    cx.engine
        .state
        .active_event_observers
        .push(ActiveEventObserver {
            watched,
            matcher,
            payload: EventObserverPayload::StageDelayedTrigger(Box::new(DelayedTriggerPayload {
                source: TriggerObjectRef {
                    object_id: cx.top.source_permanent_id.unwrap_or(cx.top.id),
                    zone_change_generation: cx
                        .top
                        .cast_occurrence
                        .and_then(|cast| cast.zone_change_generation)
                        .unwrap_or(cx.top.source_zone_change),
                    controller_at_event: cx.controller,
                },
                controller: cx.controller,
                affected_player,
                memory_jar_cohort,
                card_id: cx.top.card_id.clone(),
                card_name,
                source_face_index: cx.top.face_index,
                presentation,
                program_definition,
                ability: *ability,
            })),
        });
    Ok(EffectOutcome::Continue)
}

/// A spell-face delayed child binds to the exact printed face even when the resolving spell is a
/// copy. Ability occurrences and parentless granted abilities must keep their existing closed
/// identity rules; source objects are deliberately not consulted here.
fn is_spell_stack_item_for_delayed_child(top: &StackItem) -> bool {
    top.ability_text.is_none()
        && !top.is_triggered
        && top.ability_index.is_none()
        && top.activated_ability.is_none()
        && top.activated_ability_occurrence.is_none()
        && top.triggered_ability.is_none()
        && top.trigger_use_key.is_none()
}

fn attach_equipment_source(
    cx: &mut EffectCx<'_>,
    effect_name: &str,
) -> Result<EffectOutcome, EngineError> {
    let target_id = cx.first_current_battlefield_target(0);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let top = cx.top;
    let spell_label = cx.spell_label;

    let equip_oid = match top.source_permanent_id {
        Some(id) => id,
        None => {
            events.push(ev_log(format!(
                "{spell_label}: {effect_name} has no source permanent."
            )));
            return Ok(EffectOutcome::Continue);
        }
    };
    if !engine.source_is_current_object(top) {
        events.push(ev_log(format!(
            "{spell_label}: attachment source is no longer the same object."
        )));
        return Ok(EffectOutcome::Continue);
    }
    if let Some(target_id) = target_id {
        if super::super::targeting::equipment_attachment_legal(engine, equip_oid, target_id) {
            let tgt = object_display_name(&engine.state, engine.registry, target_id);
            let eq_name = object_display_name(&engine.state, engine.registry, equip_oid);
            let recipient = AttachmentRecipient::Object(target_id);
            let changed = engine.state.objects[&equip_oid].attached_to != Some(recipient);
            if changed {
                engine
                    .state
                    .objects
                    .get_mut(&equip_oid)
                    .expect("source exists")
                    .attached_to = Some(recipient);
                let timestamp = engine.state.command_index;
                for effect in &mut engine.state.continuous_effects {
                    if effect.source_id == Some(equip_oid)
                        && effect.duration == EffectDuration::WhileSourceOnBattlefield
                    {
                        effect.timestamp = timestamp;
                    }
                }
            }
            events.push(ev_log(format!(
                "{spell_label} attaches {eq_name} to {tgt}."
            )));
        }
    }

    Ok(EffectOutcome::Continue)
}

fn attach_equipment_subject(
    cx: &EffectCx<'_>,
    subject: &EffectSubject,
    chosen_role: &mut usize,
) -> Option<ObjectId> {
    match subject {
        EffectSubject::Chosen(_) => {
            let object = cx.first_current_battlefield_target(*chosen_role);
            *chosen_role += 1;
            object
        }
        EffectSubject::PreviousEffectObject => cx.previous_battlefield_object(),
        EffectSubject::PreviousEffectObjects => None,
        EffectSubject::Source => resolve_effect_subject(cx.engine, cx.top, &[], subject),
        EffectSubject::AttachedObject | EffectSubject::TriggerObject => None,
        EffectSubject::TargetAlias(_) => None,
    }
}

pub(super) fn attach_equipment(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::AttachEquipment {
        equipment,
        creature,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let mut chosen_role = 0;
    let Some(equipment_id) = attach_equipment_subject(cx, &equipment, &mut chosen_role) else {
        return Ok(EffectOutcome::Continue);
    };
    let Some(creature_id) = attach_equipment_subject(cx, &creature, &mut chosen_role) else {
        return Ok(EffectOutcome::Continue);
    };
    attach_equipment_ids(
        cx.engine,
        equipment_id,
        creature_id,
        cx.spell_label,
        cx.events,
    );
    Ok(EffectOutcome::Continue)
}

/// Commit one already-resolved Equipment/creature pair through the shared current attachment
/// legality check. The receipt-based Program path uses this same seam after filtering its exact
/// token cohort, while ordinary target and Vow paths keep their existing subject resolution.
pub(in crate::engine) fn attach_equipment_ids(
    engine: &mut GameEngine,
    equipment_id: ObjectId,
    creature_id: ObjectId,
    spell_label: &str,
    events: &mut Vec<rv1::RuledEvent>,
) -> bool {
    if !super::super::targeting::equipment_attachment_legal(engine, equipment_id, creature_id) {
        return false;
    }

    let recipient = AttachmentRecipient::Object(creature_id);
    if engine.state.objects[&equipment_id].attached_to == Some(recipient) {
        return true;
    }
    engine
        .state
        .objects
        .get_mut(&equipment_id)
        .expect("validated Equipment")
        .attached_to = Some(recipient);
    let timestamp = engine.state.command_index;
    for continuous in &mut engine.state.continuous_effects {
        if continuous.source_id == Some(equipment_id)
            && continuous.duration == EffectDuration::WhileSourceOnBattlefield
        {
            continuous.timestamp = timestamp;
        }
    }
    let equipment_name = object_display_name(&engine.state, engine.registry, equipment_id);
    let creature_name = object_display_name(&engine.state, engine.registry, creature_id);
    events.push(ev_log(format!(
        "{spell_label} attaches {equipment_name} to {creature_name}."
    )));
    true
}

pub(super) fn attach_source(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::AttachSource { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    attach_equipment_source(cx, "AttachSource")
}

pub(super) fn equip(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Equip { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    attach_equipment_source(cx, "equip ability")
}

pub(super) fn prevent_next_damage(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::PreventNextDamage { amount, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let target = cx
        .targets
        .first()
        .copied()
        .filter(|&target| cx.target_is_current_in_role(target, 0));
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;

    // CR 614.1a: place a damage prevention shield on the target object or player.
    if let Some(tid) = target {
        engine.add_damage_prevention(
            Some(cx.top),
            cx.spell_label,
            DamagePreventionScope::Recipient(tid),
            DamagePreventionAmount::Remaining(amount),
        );
        let tgt_name = if let Some(pi) = engine.state.player_idx(tid as i32) {
            format!("P{}", engine.state.players[pi].id)
        } else {
            object_display_name(&engine.state, engine.registry, tid)
        };
        events.push(ev_log(format!(
            "Prevention shield: the next {amount} damage to {tgt_name} is prevented."
        )));
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn prevent_all_combat_damage_turn(
    cx: &mut EffectCx<'_>,
    _effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;

    // CR 614.1a: prevent all combat damage this turn (Fog, Holy Day).
    engine.add_damage_prevention(
        Some(cx.top),
        cx.spell_label,
        DamagePreventionScope::Combat,
        DamagePreventionAmount::All,
    );
    events.push(ev_log(
        "All combat damage is prevented this turn.".to_string(),
    ));

    Ok(EffectOutcome::Continue)
}

pub(super) fn prevent_all_combat_damage_to_target_turn(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::PreventAllCombatDamageToTargetTurn { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(target_id) = cx.first_current_battlefield_target(0) else {
        return Ok(EffectOutcome::Continue);
    };
    let generation = cx
        .engine
        .state
        .zone_change_generation
        .get(&target_id)
        .copied()
        .unwrap_or(0);
    let target_name = object_display_name(&cx.engine.state, cx.engine.registry, target_id);
    cx.engine.add_damage_prevention(
        Some(cx.top),
        cx.spell_label,
        DamagePreventionScope::CombatRecipient {
            object_id: target_id,
            zone_change_generation: generation,
        },
        DamagePreventionAmount::All,
    );
    cx.events.push(ev_log(format!(
        "All combat damage that would be dealt to {target_name} this turn is prevented."
    )));

    Ok(EffectOutcome::Continue)
}

pub(super) fn prevent_all_combat_damage_by_target_turn(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::PreventAllCombatDamageByTargetTurn { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(object_id) = cx.first_current_battlefield_target(0) else {
        return Ok(EffectOutcome::Continue);
    };
    let zone_change_generation = cx
        .engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0);
    cx.engine.add_damage_prevention(
        Some(cx.top),
        cx.spell_label,
        DamagePreventionScope::CombatSource {
            object_id,
            zone_change_generation,
        },
        DamagePreventionAmount::All,
    );
    let target_name = object_display_name(&cx.engine.state, cx.engine.registry, object_id);
    cx.events.push(ev_log(format!(
        "All combat damage that would be dealt by {target_name} this turn is prevented."
    )));
    Ok(EffectOutcome::Continue)
}

pub(super) fn damage_cant_be_prevented_this_turn(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DamageCantBePreventedThisTurn = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    cx.engine
        .state
        .damage_prevention_prohibitions
        .push(DamagePreventionProhibition {
            source_id: Some(cx.top.id),
        });
    cx.events
        .push(ev_log("Damage can't be prevented this turn.".to_string()));
    Ok(EffectOutcome::Continue)
}

pub(super) fn produce_mana(
    _cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let (SpellEffectKind::ProduceMana { .. }
    | SpellEffectKind::ProduceManaFromOpponentLands { .. }
    | SpellEffectKind::ProduceManaPerSourceCounter { .. }
    | SpellEffectKind::ProduceSplitManaFromRemovedStorageCounters { .. }) = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    Ok(EffectOutcome::Continue)
}

pub(super) fn add_mana(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::AddMana { amount, retention } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(player_idx) = cx.engine.state.player_idx(cx.top.controller) else {
        return Err(EngineError::UnknownPlayer(cx.top.controller));
    };
    let player = &mut cx.engine.state.players[player_idx];
    player.mana_pool.white += amount.w;
    player.mana_pool.blue += amount.u;
    player.mana_pool.black += amount.b;
    player.mana_pool.red += amount.r;
    player.mana_pool.green += amount.g;
    player.mana_pool.colorless += amount.c;
    if retention == ManaRetention::EndOfCombat {
        player.retained_combat_mana.white += amount.w;
        player.retained_combat_mana.blue += amount.u;
        player.retained_combat_mana.black += amount.b;
        player.retained_combat_mana.red += amount.r;
        player.retained_combat_mana.green += amount.g;
        player.retained_combat_mana.colorless += amount.c;
    }
    Ok(EffectOutcome::Continue)
}

pub(super) fn regenerate(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Regenerate { subject } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let tid = if matches!(subject, EffectSubject::Chosen(_)) {
        cx.first_current_battlefield_target(0)
    } else {
        resolve_effect_subject(cx.engine, cx.top, cx.targets, &subject)
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let spell_label = cx.spell_label;
    if let Some(tid) = tid.filter(|tid| {
        engine
            .state
            .objects
            .get(tid)
            .is_some_and(|object| object.zone == Zone::Battlefield)
    }) {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        if let Some(o) = engine.state.objects.get_mut(&tid) {
            o.regeneration_shields += 1;
        }
        events.push(ev_log(format!(
            "{tgt} has a regeneration shield ({spell_label})."
        )));
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn none(
    _cx: &mut EffectCx<'_>,
    _effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    Ok(EffectOutcome::Continue)
}

pub(super) fn aura_attach(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::AuraAttach { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let top = cx.top;
    let spell_label = cx.spell_label;

    if let Some(obj) = engine.state.objects.get(&top.id) {
        if obj.zone == Zone::Battlefield {
            let Some(recipient) = obj.attached_to else {
                return Ok(EffectOutcome::Continue);
            };
            let tgt = match recipient {
                AttachmentRecipient::Object(object_id) => {
                    object_display_name(&engine.state, engine.registry, object_id)
                }
                AttachmentRecipient::Player(player_id) => format!("P{player_id}"),
            };
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::AuraAttached(rv1::AuraAttached {
                    aura_object_id: top.id,
                    attachment_recipient: Some(attachment_recipient_proto(recipient)),
                })),
            });
            events.push(ev_log(format!("{spell_label} attaches to {tgt}.")));
        }
    }

    Ok(EffectOutcome::Continue)
}

#[cfg(test)]
mod delayed_trigger_tests {
    use super::*;

    fn engine(seed: u64) -> GameEngine {
        GameEngine::new_with_default_decks(tricerules_cards::registry::global(), seed, &[0, 1], 20)
            .expect("delayed-trigger runtime fixture engine")
    }

    fn delayed_spell_item() -> StackItem {
        StackItem {
            mana_colors_spent_to_cast: Default::default(),
            id: 900_001,
            controller: 0,
            card_id: "arcane_denial".into(),
            prototyped: false,
            targets: Vec::new(),
            ability_text: None,
            source_permanent_id: None,
            source_owner: Some(0),
            source_zone_change: 0,
            trigger_use_key: None,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            activated_ability_occurrence: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index: 0,
            cast_method: crate::state::SpellCastMethod::Normal,
            returned_attacker_assignment: None,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            cast_cost_receipts: Vec::new(),
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: Some(0),
            payment_result: Default::default(),
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: Default::default(),
        }
    }

    fn delayed_child_effect() -> SpellEffectKind {
        let face = tricerules_cards::registry::global()
            .get("arcane_denial")
            .expect("Arcane Denial definition")
            .primary_face();
        let program = face.spell_program.as_ref().expect("Arcane spell Program");
        let delayed = program
            .body
            .steps
            .get(1)
            .expect("targeted delayed trigger instruction");
        let tricerules_cards::primitives::InstructionKind::Perform(effect) = &delayed.kind else {
            panic!("the targeted delayed trigger is a performed effect");
        };
        effect.as_ref().clone()
    }

    fn invoke_delayed_child(
        engine: &mut GameEngine,
        top: &StackItem,
        effect: SpellEffectKind,
        target_controller: Option<i32>,
    ) -> Result<EffectOutcome, EngineError> {
        let previous_result = crate::state::EffectResult {
            program_value: Some(crate::state::ProgramValue::TargetedSpellController(
                target_controller,
            )),
            ..Default::default()
        };
        let mut effect_result = crate::state::EffectResult::default();
        let mut events = Vec::new();
        let mut cx = EffectCx {
            engine,
            events: &mut events,
            targets: &[],
            targets_by_role: &[],
            target_receipts: &[],
            target_role_group_indices: &[],
            top,
            controller: top.controller,
            affected_player: top.controller,
            spell_label: "Arcane Denial",
            previous_effect_result: &previous_result,
            resume_previous_result: Some(&previous_result),
            effect_result: &mut effect_result,
            effect_index: 1,
            program_state: None,
        };
        create_delayed_trigger(&mut cx, effect)
    }

    #[test]
    fn typed_none_target_controller_continues_without_an_observer() {
        let mut engine = engine(91_001);
        let top = delayed_spell_item();
        let before = engine.diagnostic_snapshot().unwrap();

        let result = invoke_delayed_child(&mut engine, &top, delayed_child_effect(), None);

        assert!(matches!(result, Ok(EffectOutcome::Continue)));
        assert!(engine.state.active_event_observers.is_empty());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }

    #[test]
    fn missing_compiled_child_owner_errors_before_observer_publication() {
        let mut engine = engine(91_002);
        let top = delayed_spell_item();
        let mut effect = delayed_child_effect();
        let SpellEffectKind::CreateDelayedTrigger { ability, .. } = &mut effect else {
            panic!("the Arcane delayed-child fixture is a CreateDelayedTrigger")
        };
        ability.ability_id =
            tricerules_cards::AbilityId::new("missing_child").expect("fixture child ability id");
        let before = engine.diagnostic_snapshot().unwrap();

        let result = invoke_delayed_child(&mut engine, &top, effect, Some(1));

        assert!(matches!(
            result,
            Err(EngineError::ProgramInvariant(
                "authored delayed child has no exact compiled Program owner"
            ))
        ));
        assert!(engine.state.active_event_observers.is_empty());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }

    #[test]
    fn unrelated_parentless_ability_cannot_claim_a_face_spell_child_owner() {
        let mut engine = engine(91_003);
        let mut top = delayed_spell_item();
        top.ability_text = Some("unrelated trigger".into());
        top.is_triggered = true;
        top.ability_index = Some(0);
        let before = engine.diagnostic_snapshot().unwrap();

        let result = invoke_delayed_child(&mut engine, &top, delayed_child_effect(), Some(1));

        assert!(matches!(
            result,
            Err(EngineError::ProgramInvariant(
                "authored delayed child has no immutable parent ability definition"
            ))
        ));
        assert!(engine.state.active_event_observers.is_empty());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
}
