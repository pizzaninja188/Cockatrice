use super::super::events::ev_log_private;
use super::candidate_identities;
use super::*;

pub(super) fn draw(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Draw { count } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let top = cx.top;
    // "That player draws…" (Howling Mine) resolves against the trigger's affected player; for a
    // spell or an ordinary ability this *is* the controller.
    let drawer = cx.affected_player;
    let spell_label = cx.spell_label;

    // Blue Sun's Zenith / Braingeyser: `count` may be the cast-time X.
    let count = engine.resolve_amount(
        &count,
        AmountContext::for_stack_item(top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    draw_cards_for_player(engine, events, drawer, count, spell_label)?;

    Ok(EffectOutcome::Continue)
}

pub(in crate::engine) fn draw_cards_for_player(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    drawer: PlayerId,
    count: u32,
    spell_label: &str,
) -> Result<(), EngineError> {
    let idx = engine
        .state
        .player_idx(drawer)
        .ok_or(EngineError::Illegal("draw recipient not found"))?;
    // CR 120.3 / 104.3c: drawing from an empty library does NOT fail the spell —
    // draw as many as possible, then the player loses as a state-based action
    // (swept in by `sweep_life`). Aborting resolution here would corrupt state
    // (cards already drawn, stack already popped).
    let mut drawn = 0u32;
    let mut decked_out = false;
    for _ in 0..count {
        if engine.state.players[idx].library.is_empty() {
            decked_out = true;
            break;
        }
        draw_card(&mut engine.state.players[idx], &mut engine.state.objects)?;
        drawn += 1;
    }
    let noun = if drawn == 1 { "card" } else { "cards" };
    events.push(ev_log(format!(
        "P{drawer} draws {drawn} {noun} ({spell_label})."
    )));
    if decked_out {
        engine.state.players[idx].has_lost = true;
        events.push(ev_log(format!(
            "P{drawer} tried to draw from an empty library and loses (CR 104.3c)."
        )));
    }

    Ok(())
}

pub(super) fn exile_target(
    cx: &mut EffectCx<'_>,
    _effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        let owner = engine.state.objects.get(&tid).map(|o| o.owner);
        move_object_to_zone(&mut engine.state, engine.registry, tid, Zone::Exile, None)?;
        events.push(ev_log(format!("{spell_label} exiles {tgt}")));
        if let Some(owner_id) = owner {
            events.push(permanent_moved_event(
                &engine.state,
                tid,
                owner_id,
                rv1::permanent_moved::Destination::Exile,
            ));
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn exile_if_would_die_this_turn(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ExileIfWouldDieThisTurn { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(&object_id) = cx.targets.first() else {
        return Ok(EffectOutcome::Continue);
    };
    if !cx
        .engine
        .state
        .objects
        .get(&object_id)
        .is_some_and(|object| object.zone == Zone::Battlefield)
    {
        return Ok(EffectOutcome::Continue);
    }
    let zone_change_generation = cx
        .engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0);
    let replacement = ActiveDeathReplacement {
        object_id,
        zone_change_generation,
    };
    if !cx
        .engine
        .state
        .death_replacement_effects
        .contains(&replacement)
    {
        cx.engine.state.death_replacement_effects.push(replacement);
    }
    Ok(EffectOutcome::Continue)
}

pub(super) fn exile_target_gain_life_equal_to_power(
    cx: &mut EffectCx<'_>,
    _effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        // CR 608: read effective power at resolution before the object leaves.
        let power = engine.effective_power(tid).unwrap_or(0);
        let owner = engine.state.objects.get(&tid).map(|o| o.owner);
        let target_controller = engine.state.objects.get(&tid).map(|o| o.controller);
        let target_controller = target_controller.unwrap_or(controller);
        move_object_to_zone(&mut engine.state, engine.registry, tid, Zone::Exile, None)?;
        events.push(ev_log(format!("{spell_label} exiles {tgt}")));
        if let Some(owner_id) = owner {
            events.push(permanent_moved_event(
                &engine.state,
                tid,
                owner_id,
                rv1::permanent_moved::Destination::Exile,
            ));
        }
        super::life::apply_life_gain(engine, events, target_controller, power, spell_label);
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn return_to_owners_hand(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ReturnToOwnersHand { subject } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let tid = resolve_effect_subject(cx.engine, cx.top, cx.targets, &subject);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let spell_label = cx.spell_label;

    if let Some(tid) = tid {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        let owner = engine.state.objects.get(&tid).map(|o| o.owner);
        // Transient battlefield state (damage, counters, tap) is reset centrally
        // by move_object_to_zone on leaving the battlefield (CR 400.7 / 121.2).
        move_object_to_zone(&mut engine.state, engine.registry, tid, Zone::Hand, None)?;
        events.push(ev_log(format!(
            "{spell_label} returns {tgt} to its owner's hand"
        )));
        if let Some(owner_id) = owner {
            events.push(permanent_moved_event(
                &engine.state,
                tid,
                owner_id,
                rv1::permanent_moved::Destination::Hand,
            ));
        }
    }

    Ok(EffectOutcome::Continue)
}

fn move_permanent_to_owners_library(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    tid: ObjectId,
    placement: LibraryPlacement,
    spell_label: &str,
) -> Result<(), EngineError> {
    let target_name = object_display_name(&engine.state, engine.registry, tid);
    let owner = engine
        .state
        .objects
        .get(&tid)
        .map(|object| object.owner)
        .ok_or(EngineError::Illegal("no target object"))?;

    move_object_to_zone(&mut engine.state, engine.registry, tid, Zone::Library, None)?;
    let owner_idx = engine
        .state
        .player_idx(owner)
        .ok_or(EngineError::UnknownPlayer(owner))?;
    match placement {
        LibraryPlacement::Top => {
            engine.state.players[owner_idx]
                .library
                .retain(|&oid| oid != tid);
            engine.state.players[owner_idx].library.push_front(tid);
            events.push(ev_log(format!(
                "{spell_label} puts {target_name} on top of its owner's library."
            )));
        }
        LibraryPlacement::Bottom => {
            events.push(ev_log(format!(
                "{spell_label} puts {target_name} on the bottom of its owner's library."
            )));
        }
        LibraryPlacement::Shuffle => {
            crate::engine::shuffle_player_library_for_current_command(&mut engine.state, owner);
            events.push(ev_log(format!("P{owner} shuffles their library.")));
        }
    }
    events.push(permanent_moved_event(
        &engine.state,
        tid,
        owner,
        rv1::permanent_moved::Destination::Library,
    ));
    Ok(())
}

pub(super) fn put_target_permanent_in_owners_library(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::PutTargetPermanentInOwnersLibrary {
        target: _,
        placement,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    if let Some(&tid) = cx.targets.first() {
        move_permanent_to_owners_library(cx.engine, cx.events, tid, placement, cx.spell_label)?;
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn discard_cards(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DiscardCards {
        count,
        target: _,
        chooser,
        card_filter,
        optional,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(&target) = cx.targets.first() else {
        return Ok(EffectOutcome::Continue);
    };
    discard_for_player(
        cx,
        target as PlayerId,
        count,
        chooser,
        card_filter.as_ref(),
        optional,
        0,
    )
}

pub(super) fn draw_discard(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DrawDiscard {
        who,
        draw_count,
        discard_count,
        order,
        optional,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let recipients = player_recipients(cx, who);
    let [player] = recipients.as_slice() else {
        return if recipients.is_empty() {
            Ok(EffectOutcome::Continue)
        } else {
            Err(EngineError::Illegal(
                "DrawDiscard requires exactly one affected player",
            ))
        };
    };
    let player = *player;

    match order {
        DrawDiscardOrder::DrawThenDiscard => {
            draw_cards_for_player(cx.engine, cx.events, player, draw_count, cx.spell_label)?;
            discard_for_player(
                cx,
                player,
                discard_count,
                DiscardChooser::AffectedPlayer,
                None,
                false,
                0,
            )
        }
        DrawDiscardOrder::DiscardThenDraw => discard_for_player(
            cx,
            player,
            discard_count,
            DiscardChooser::AffectedPlayer,
            None,
            optional,
            draw_count,
        ),
    }
}

fn discard_for_player(
    cx: &mut EffectCx<'_>,
    affected_player: PlayerId,
    count: u32,
    chooser: DiscardChooser,
    card_filter: Option<&CardTypeFilter>,
    optional: bool,
    draw_after: u32,
) -> Result<EffectOutcome, EngineError> {
    let controller = cx.controller;
    let top = cx.top;
    let spell_label = cx.spell_label;
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let Some(pi) = engine.state.player_idx(affected_player) else {
        return Ok(EffectOutcome::Continue);
    };
    let hand = engine.state.players[pi].hand.clone();
    let eligible: Vec<ObjectId> = hand
        .iter()
        .copied()
        .filter(|object_id| {
            card_matches_type_filter(&engine.state, engine.registry, *object_id, card_filter)
        })
        .collect();

    if chooser == DiscardChooser::Random {
        let discard_count = (count as usize).min(eligible.len());
        if discard_count == 0 {
            events.push(ev_log(format!(
                "P{affected_player} has no cards to discard ({spell_label})."
            )));
        } else {
            let mut shuffled = eligible;
            shuffle_object_ids_for_current_command(&engine.state, affected_player, &mut shuffled);
            for oid in shuffled.into_iter().take(discard_count) {
                let card_name = object_display_name(&engine.state, engine.registry, oid);
                move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    oid,
                    Zone::Graveyard,
                    None,
                )?;
                events.push(permanent_moved_event(
                    &engine.state,
                    oid,
                    affected_player,
                    rv1::permanent_moved::Destination::Graveyard,
                ));
                events.push(ev_log(format!(
                    "P{affected_player} discards {card_name} ({spell_label})."
                )));
            }
        }
        return Ok(EffectOutcome::Continue);
    }

    if hand.is_empty() {
        events.push(ev_log(format!(
            "P{affected_player} has no cards to discard ({spell_label})."
        )));
        if draw_after > 0 && !optional {
            draw_cards_for_player(engine, events, affected_player, draw_after, spell_label)?;
        }
        return Ok(EffectOutcome::Continue);
    }

    let n = (eligible.len() as u32).min(count);
    let min = if optional { 0 } else { n };
    let deciding_player = match chooser {
        DiscardChooser::AffectedPlayer => affected_player,
        DiscardChooser::Controller => controller,
        DiscardChooser::Random => unreachable!("handled above"),
    };
    let choice_kind = match chooser {
        DiscardChooser::AffectedPlayer => custom::ChoiceKind::HandCards,
        DiscardChooser::Controller => custom::ChoiceKind::OpponentHand,
        DiscardChooser::Random => unreachable!("handled above"),
    };
    let (candidate_card_ids, candidate_names) = candidate_identities(engine, &hand);
    let candidate_selectable = hand
        .iter()
        .map(|object_id| eligible.contains(object_id))
        .collect();
    let prompt = if optional {
        format!("P{deciding_player}: you may choose a card for P{affected_player} to discard.")
    } else {
        format!("P{deciding_player}: choose {n} card(s) for P{affected_player} to discard.")
    };
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: deciding_player,
                source_object_id: top.id,
                prompt_text: prompt.clone(),
                choice_kind: choice_kind as i32,
                candidate_object_ids: hand.clone(),
                candidate_card_ids,
                candidate_names,
                min,
                max: n,
                ordered: false,
                unique_names: false,
                candidate_server_card_ids: Vec::new(),
                candidate_selectable,
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                generic_mana_cost: 0,
                payment_currently_legal: false,
            },
        )),
    });
    events.push(ev_log(prompt.clone()));
    engine.state.pending_resolution = Some(PendingResolution {
        deciding_player,
        presentation: PendingResolutionPresentation {
            source_object_id: top.id,
            candidates: eligible,
            min,
            max: n,
            ordered: false,
            unique_names: false,
            prompt,
            choice_kind,
        },
        continuation: ResolutionContinuation::Discard {
            stack: ParkedStackResolution::new(top.clone()),
            discard: PendingDiscard {
                affected_player,
                draw_after,
                draw_only_if_discarded: optional,
            },
        },
    });
    Ok(EffectOutcome::Suspended)
}

pub(super) fn mill_target_player(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::MillTargetPlayer { count, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let recipients = cx.targets.first().map(|target| *target as PlayerId);
    if let Some(recipient) = recipients {
        let milled = mill_players(cx, &[recipient], count);
        *cx.effect_result = EffectResult::MilledCards(milled);
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn mill(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Mill { count, who } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let amount_context = AmountContext::for_stack_item(cx.top, cx.controller)
        .with_previous_effect_result(cx.previous_effect_result);
    let count = cx.engine.resolve_amount(&count, amount_context);
    let recipients = player_recipients(cx, who);
    let milled = mill_players(cx, &recipients, count);
    *cx.effect_result = EffectResult::MilledCards(milled);

    Ok(EffectOutcome::Continue)
}

fn mill_players(cx: &mut EffectCx<'_>, recipients: &[PlayerId], count: u32) -> Vec<ObjectId> {
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let spell_label = cx.spell_label;
    let mut result = vec![];

    for &pid in recipients {
        let Some(pi) = engine.state.player_idx(pid) else {
            continue;
        };
        let mut milled = 0u32;
        for _ in 0..count {
            let Some(oid) = engine.state.players[pi].library.pop_front() else {
                break;
            };
            engine.state.players[pi].graveyard.push(oid);
            if let Some(o) = engine.state.objects.get_mut(&oid) {
                o.zone = Zone::Graveyard;
            }
            events.push(permanent_moved_event(
                &engine.state,
                oid,
                pid,
                rv1::permanent_moved::Destination::Graveyard,
            ));
            result.push(oid);
            milled += 1;
        }
        events.push(ev_log(format!(
            "{spell_label} mills {milled} card(s) from P{pid}"
        )));
    }

    result
}

pub(super) fn target_player_sacrifices(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::TargetPlayerSacrifices { filter, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            // Collect matching permanents on the target player's battlefield.
            let qualifying: Vec<ObjectId> = engine.state.players[pi]
                .battlefield
                .iter()
                .copied()
                .filter(|&oid| object_matches_mass_filter(engine, oid, &filter))
                .collect();
            if qualifying.is_empty() {
                events.push(ev_log(format!(
                    "P{pid} has no valid permanents to sacrifice ({spell_label})."
                )));
            } else {
                let candidate_card_ids: Vec<String> = qualifying
                    .iter()
                    .map(|&oid| {
                        engine
                            .state
                            .objects
                            .get(&oid)
                            .map(|o| o.card_id.clone())
                            .unwrap_or_default()
                    })
                    .collect();
                let candidate_names: Vec<String> = candidate_card_ids
                    .iter()
                    .map(|cid| {
                        engine
                            .registry
                            .get(cid)
                            .map(|d| d.name.clone())
                            .unwrap_or_else(|| cid.clone())
                    })
                    .collect();
                let prompt = format!("P{pid}: choose a permanent to sacrifice ({spell_label}).");
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                        rv1::ResolutionChoiceRequired {
                            deciding_player_id: pid,
                            source_object_id: top.id,
                            prompt_text: prompt.clone(),
                            // Revealed: the battlefield is public so no
                            // hidden-zone redaction is needed.
                            choice_kind: custom::ChoiceKind::Revealed as i32,
                            candidate_object_ids: qualifying.clone(),
                            candidate_card_ids,
                            candidate_names,
                            min: 1,
                            max: 1,
                            ordered: false,
                            unique_names: false,
                            candidate_server_card_ids: Vec::new(),
                            candidate_selectable: Vec::new(),
                            resolution_branches: Vec::new(),
                            mana_cost: String::new(),
                            generic_mana_cost: 0,
                            payment_currently_legal: false,
                        },
                    )),
                });
                events.push(ev_log(prompt.clone()));
                engine.state.pending_resolution = Some(PendingResolution {
                    deciding_player: pid,
                    presentation: PendingResolutionPresentation {
                        source_object_id: top.id,
                        candidates: qualifying,
                        min: 1,
                        max: 1,
                        ordered: false,
                        unique_names: false,
                        prompt,
                        choice_kind: custom::ChoiceKind::Revealed,
                    },
                    continuation: ResolutionContinuation::Sacrifice {
                        stack: ParkedStackResolution::new(top.clone()),
                    },
                });
                return Ok(EffectOutcome::Suspended);
            }
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn return_from_graveyard(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ReturnFromGraveyard {
        filter,
        destination,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let item = cx.top.clone();
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    if destination == tricerules_cards::primitives::GraveyardDestination::Hand {
        for &tid in targets {
            let target_name = object_display_name(&engine.state, engine.registry, tid);
            let owner = engine.state.objects.get(&tid).map(|object| object.owner);
            move_object_to_zone(&mut engine.state, engine.registry, tid, Zone::Hand, None)?;
            events.push(ev_log(format!(
                "{spell_label} returns {target_name} from graveyard to hand."
            )));
            if let Some(owner_id) = owner {
                events.push(permanent_moved_event(
                    &engine.state,
                    tid,
                    owner_id,
                    rv1::permanent_moved::Destination::Hand,
                ));
            }
        }
        return Ok(EffectOutcome::Continue);
    }

    if let Some(&tid) = targets.first() {
        let tgt = object_display_name(&engine.state, engine.registry, tid);
        let is_legal = graveyard_target_legal(engine, &filter, tid, controller);
        if !is_legal {
            events.push(ev_log(format!(
                "{spell_label} fizzles: {tgt} is no longer a legal graveyard target."
            )));
        } else {
            let owner = engine.state.objects.get(&tid).map(|o| o.owner);
            use tricerules_cards::primitives::GraveyardDestination;
            let dest_zone = match destination {
                GraveyardDestination::Hand => Zone::Hand,
                GraveyardDestination::Battlefield => Zone::Battlefield,
            };
            let dest_proto = match destination {
                GraveyardDestination::Hand => rv1::permanent_moved::Destination::Hand,
                GraveyardDestination::Battlefield => rv1::permanent_moved::Destination::Battlefield,
            };
            // CR 110.2: a card put onto the battlefield by an effect enters under the controller
            // of that effect ("under your control"), not under its owner's control. Irrelevant
            // for `GraveyardOwner::Controller` cards (Zombify), load-bearing for `AnyPlayer`
            // reanimation out of an opponent's graveyard.
            if dest_zone == Zone::Battlefield {
                let deciding_player = owner.unwrap_or(controller);
                match engine.begin_battlefield_entry(
                    item,
                    BattlefieldEntryEvent {
                        object_id: tid,
                        deciding_player,
                        destination_controller: controller,
                        face_index: 0,
                        unlock_room_door: None,
                        chosen_x: 0,
                        player_life_snapshot: engine.player_life_snapshot(),
                        tapped: false,
                        entry_counters: BTreeMap::new(),
                        applied_effects: Vec::new(),
                    },
                    BattlefieldEntryCompletion::ResolutionEffect {
                        owner: deciding_player,
                        spell_label: spell_label.to_string(),
                        object_label: tgt.clone(),
                    },
                    events,
                ) {
                    super::super::replacement::BattlefieldEntryProgress::Parked => {
                        return Ok(EffectOutcome::Suspended);
                    }
                    super::super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                        engine.commit_battlefield_entry(entry, None)?;
                    }
                }
            } else {
                move_object_to_zone(&mut engine.state, engine.registry, tid, dest_zone, None)?;
            }
            let dest_name = match destination {
                GraveyardDestination::Hand => "hand",
                GraveyardDestination::Battlefield => "battlefield",
            };
            events.push(ev_log(format!(
                "{spell_label} returns {tgt} from graveyard to {dest_name}."
            )));
            if let Some(owner_id) = owner {
                events.push(permanent_moved_event(
                    &engine.state,
                    tid,
                    owner_id,
                    dest_proto,
                ));
            }
        }
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn return_triggered_card_from_graveyard(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ReturnTriggeredCardFromGraveyard {
        reference,
        tapped,
        controller,
        entry_counters,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let (source_id, event_generation) = match reference {
        TriggeredCardReference::AbilitySource => {
            let Some(source_id) = cx.top.source_permanent_id else {
                return Ok(EffectOutcome::Continue);
            };
            (source_id, cx.top.source_zone_change)
        }
        TriggeredCardReference::TriggerObject => {
            let Some(observed) = cx.top.trigger_context.observed_object else {
                return Ok(EffectOutcome::Continue);
            };
            (observed.object_id, observed.zone_change_generation)
        }
    };
    let current_generation = cx
        .engine
        .state
        .zone_change_generation
        .get(&source_id)
        .copied()
        .unwrap_or(0);
    let Some(object) = cx.engine.state.objects.get(&source_id) else {
        return Ok(EffectOutcome::Continue);
    };
    if object.zone != Zone::Graveyard || current_generation != event_generation.saturating_add(1) {
        return Ok(EffectOutcome::Continue);
    }

    let owner = object.owner;
    let destination_controller = match controller {
        ReturnController::Owner => owner,
        ReturnController::AbilityController => cx.controller,
    };
    let object_label = object_display_name(&cx.engine.state, cx.engine.registry, source_id);
    let entry_counters = entry_counters
        .into_iter()
        .map(|placement| (placement.counter, placement.count))
        .collect();
    match cx.engine.begin_battlefield_entry(
        cx.top.clone(),
        BattlefieldEntryEvent {
            object_id: source_id,
            deciding_player: destination_controller,
            destination_controller,
            face_index: 0,
            unlock_room_door: None,
            chosen_x: 0,
            player_life_snapshot: cx.engine.player_life_snapshot(),
            tapped,
            entry_counters,
            applied_effects: Vec::new(),
        },
        BattlefieldEntryCompletion::ResolutionEffect {
            owner,
            spell_label: cx.spell_label.to_string(),
            object_label: object_label.clone(),
        },
        cx.events,
    ) {
        super::super::replacement::BattlefieldEntryProgress::Parked => Ok(EffectOutcome::Suspended),
        super::super::replacement::BattlefieldEntryProgress::Ready(entry) => {
            cx.engine.commit_battlefield_entry(entry, None)?;
            cx.events.push(ev_log(format!(
                "{} returns {object_label} from graveyard to battlefield.",
                cx.spell_label
            )));
            cx.events.push(permanent_moved_event(
                &cx.engine.state,
                source_id,
                owner,
                rv1::permanent_moved::Destination::Battlefield,
            ));
            Ok(EffectOutcome::Continue)
        }
    }
}

/// CR 701.18: look at the top N cards of your library, then decide which go to the bottom and in
/// what order the rest sit on top.
///
/// Parks the first of up to two interrupts (see `finish_scry` for the second). The cards stay in
/// the library throughout — scry reorders a hidden zone, it does not move cards through zones — so
/// nothing here touches `move_object_to_zone` and no zone-change trigger can see it. Only the
/// *count* is public; the card names go to the scrying player alone.
pub(super) fn scry(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Scry { count } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let top = cx.top;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    let Some(idx) = engine.state.player_idx(controller) else {
        return Ok(EffectOutcome::Continue);
    };
    // CR 701.18b: scrying fewer cards than asked for is legal — you look at as many as there are.
    let candidates: Vec<ObjectId> = engine.state.players[idx]
        .library
        .iter()
        .take(count as usize)
        .copied()
        .collect();
    let n = candidates.len() as u32;
    if n == 0 {
        events.push(ev_log(format!(
            "P{controller} scries {count} with an empty library ({spell_label})."
        )));
        return Ok(EffectOutcome::Continue);
    }

    let (candidate_card_ids, candidate_names) = candidate_identities(engine, &candidates);
    let prompt = format!(
        "Scry {n}: click any number of cards to put on the bottom of your library — they go down \
         in the order you click them, so the last one ends up bottom-most. The rest stay on top."
    );
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: controller,
                source_object_id: top.id,
                prompt_text: prompt.clone(),
                choice_kind: custom::ChoiceKind::LibraryTop as i32,
                candidate_object_ids: candidates.clone(),
                candidate_card_ids,
                candidate_names: candidate_names.clone(),
                min: 0,
                max: n,
                // CR 701.18a puts the bottomed cards down "in any order", and `finish_scry`
                // seats them in submitted order — so the order is load-bearing here, not
                // incidental.
                ordered: true,
                unique_names: false,
                candidate_server_card_ids: Vec::new(),
                candidate_selectable: Vec::new(),
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                generic_mana_cost: 0,
                payment_currently_legal: false,
            },
        )),
    });
    // The count is public knowledge; what was seen is not.
    events.push(ev_log(format!("P{controller} scries {n} ({spell_label}).")));
    events.push(ev_log_private(
        format!("P{controller} looks at {}.", candidate_names.join(", ")),
        controller,
    ));
    engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: controller,
        presentation: PendingResolutionPresentation {
            source_object_id: top.id,
            candidates: candidates.clone(),
            min: 0,
            max: n,
            ordered: true,
            unique_names: false,
            prompt,
            choice_kind: custom::ChoiceKind::LibraryTop,
        },
        continuation: ResolutionContinuation::Scry {
            stack: ParkedStackResolution::new(top.clone()),
            looked_at: candidates,
            stage: PendingScryStage::ChooseBottom,
        },
    });
    Ok(EffectOutcome::Suspended)
}

/// CR 701.62: look at the top two cards, manifest one, and put the other into the graveyard.
/// The two-card branch parks behind the existing private library image picker; short libraries
/// complete deterministically without asking a meaningless question.
pub(super) fn manifest_dread(cx: &mut EffectCx<'_>) -> Result<EffectOutcome, EngineError> {
    let engine = &mut *cx.engine;
    let controller = cx.controller;
    let Some(player_idx) = engine.state.player_idx(controller) else {
        return Ok(EffectOutcome::Continue);
    };
    let candidates: Vec<ObjectId> = engine.state.players[player_idx]
        .library
        .iter()
        .take(2)
        .copied()
        .collect();
    if candidates.is_empty() {
        cx.events.push(ev_log(format!(
            "P{controller} manifests dread with an empty library ({}).",
            cx.spell_label
        )));
        return Ok(EffectOutcome::Continue);
    }
    if candidates.len() == 1 {
        let object_id = candidates[0];
        engine
            .state
            .objects
            .get_mut(&object_id)
            .expect("library object")
            .face_down = true;
        match engine.begin_battlefield_entry(
            cx.top.clone(),
            BattlefieldEntryEvent {
                object_id,
                deciding_player: controller,
                destination_controller: controller,
                face_index: 0,
                unlock_room_door: None,
                chosen_x: 0,
                player_life_snapshot: engine.player_life_snapshot(),
                tapped: false,
                entry_counters: BTreeMap::new(),
                applied_effects: Vec::new(),
            },
            BattlefieldEntryCompletion::ManifestDread {
                owner: controller,
                other_object_id: None,
                chosen_library_position: 0,
            },
            cx.events,
        ) {
            super::super::replacement::BattlefieldEntryProgress::Parked => {
                return Ok(EffectOutcome::Suspended);
            }
            super::super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                engine.commit_battlefield_entry(entry, None)?;
                cx.events.push(permanent_moved_event_with_library_position(
                    &engine.state,
                    object_id,
                    controller,
                    rv1::permanent_moved::Destination::Battlefield,
                    0,
                ));
                cx.events
                    .push(ev_log(format!("P{controller} manifests dread.")));
                return Ok(EffectOutcome::Continue);
            }
        }
    }

    let (candidate_card_ids, candidate_names) = candidate_identities(engine, &candidates);
    let prompt =
        "Choose one of the top two cards to manifest. The other will be put into your graveyard."
            .to_string();
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: controller,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: custom::ChoiceKind::ManifestDread as i32,
                candidate_object_ids: candidates.clone(),
                candidate_card_ids,
                candidate_names: candidate_names.clone(),
                min: 1,
                max: 1,
                ordered: false,
                unique_names: false,
                candidate_server_card_ids: Vec::new(),
                candidate_selectable: Vec::new(),
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                generic_mana_cost: 0,
                payment_currently_legal: false,
            },
        )),
    });
    cx.events.push(ev_log(format!(
        "P{controller} looks at the top two cards to manifest dread ({}).",
        cx.spell_label
    )));
    cx.events.push(ev_log_private(
        format!("P{controller} looks at {}.", candidate_names.join(", ")),
        controller,
    ));
    engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: controller,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates: candidates.clone(),
            min: 1,
            max: 1,
            ordered: false,
            unique_names: false,
            prompt,
            choice_kind: custom::ChoiceKind::ManifestDread,
        },
        continuation: ResolutionContinuation::ManifestDread {
            stack: ParkedStackResolution::new(cx.top.clone()),
            looked_at: candidates,
        },
    });
    Ok(EffectOutcome::Suspended)
}

/// Look at a bounded top-of-library window, show every card image privately, and let the
/// controller choose at most one matching card. Selection legality stays engine-authored through
/// `candidate_selectable`; the client never derives it from display/Oracle data.
pub(super) fn look_choose_to_hand(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::LookChooseToHand {
        count,
        filter,
        bottom_order,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let controller = cx.controller;
    let Some(idx) = engine.state.player_idx(controller) else {
        return Ok(EffectOutcome::Continue);
    };
    let looked: Vec<ObjectId> = engine.state.players[idx]
        .library
        .iter()
        .take(count as usize)
        .copied()
        .collect();
    if looked.is_empty() {
        cx.events.push(ev_log(format!(
            "P{controller} looks at an empty library ({}).",
            cx.spell_label
        )));
        return Ok(EffectOutcome::Continue);
    }

    let selectable: Vec<bool> = looked
        .iter()
        .map(|&oid| card_matches_type_filter(&engine.state, engine.registry, oid, Some(&filter)))
        .collect();
    let legal: Vec<ObjectId> = looked
        .iter()
        .copied()
        .zip(&selectable)
        .filter_map(|(oid, selectable)| selectable.then_some(oid))
        .collect();
    let (candidate_card_ids, candidate_names) = candidate_identities(engine, &looked);
    let n = looked.len() as u32;
    let prompt = format!(
        "Look at the top {n} cards. Click up to one matching card image to reveal and put into your hand."
    );
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: controller,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: custom::ChoiceKind::LibraryLook as i32,
                candidate_object_ids: looked.clone(),
                candidate_card_ids,
                candidate_names: candidate_names.clone(),
                min: 0,
                max: 1,
                ordered: false,
                unique_names: false,
                candidate_server_card_ids: Vec::new(),
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                generic_mana_cost: 0,
                payment_currently_legal: false,
                candidate_selectable: selectable,
            },
        )),
    });
    cx.events.push(ev_log(format!(
        "P{controller} looks at the top {n} cards of their library ({}).",
        cx.spell_label
    )));
    cx.events.push(ev_log_private(
        format!("P{controller} looks at {}.", candidate_names.join(", ")),
        controller,
    ));
    engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: controller,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates: legal,
            min: 0,
            max: 1,
            ordered: false,
            unique_names: false,
            prompt,
            choice_kind: custom::ChoiceKind::LibraryLook,
        },
        continuation: ResolutionContinuation::LibraryLook {
            stack: ParkedStackResolution::new(cx.top.clone()),
            stage: PendingLibraryLookStage::ChooseToHand {
                looked_at: looked,
                bottom_order,
            },
        },
    });
    Ok(EffectOutcome::Suspended)
}

pub(super) fn search_library(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::SearchLibrary {
        filter,
        destination,
        shuffle,
        reveal,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let top = cx.top;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    let candidates: Vec<ObjectId> = {
        let Some(idx) = engine.state.player_idx(controller) else {
            events.push(ev_log(format!("{spell_label} resolves (no library).")));
            return Ok(EffectOutcome::Suspended);
        };
        engine.state.players[idx]
            .library
            .iter()
            .copied()
            .filter(|&oid| {
                library_card_matches_filter(&engine.state, engine.registry, oid, filter.as_ref())
            })
            .collect()
    };
    // When a hidden-zone search requires a stated quality, the searching player may fail to find
    // even when a matching card is present. An unrestricted search must still find a card when
    // the library is nonempty.
    let min = if filter.is_some() || candidates.is_empty() {
        0u32
    } else {
        1u32
    };
    let prompt = match &filter {
        None => format!("P{controller}: search your library for a card."),
        Some(f) => format!(
            "P{controller}: search your library for a {} card.",
            library_card_filter_desc(f)
        ),
    };
    let (candidate_card_ids, candidate_names) = candidate_identities(engine, &candidates);
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: controller,
                source_object_id: top.id,
                prompt_text: prompt.clone(),
                choice_kind: custom::ChoiceKind::LibrarySearch as i32,
                candidate_object_ids: candidates.clone(),
                candidate_card_ids,
                candidate_names,
                min,
                max: 1,
                ordered: false,
                unique_names: false,
                candidate_server_card_ids: Vec::new(),
                candidate_selectable: Vec::new(),
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                generic_mana_cost: 0,
                payment_currently_legal: false,
            },
        )),
    });
    events.push(ev_log(prompt.clone()));
    engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: controller,
        presentation: PendingResolutionPresentation {
            source_object_id: top.id,
            candidates,
            min,
            max: 1,
            ordered: false,
            unique_names: false,
            prompt,
            choice_kind: custom::ChoiceKind::LibrarySearch,
        },
        continuation: ResolutionContinuation::SearchLibrary {
            stack: ParkedStackResolution::new(top.clone()),
            destination,
            shuffle,
            reveal,
        },
    });
    // Resolution is now parked; the "resolves." log is emitted by finish_library_search.
    Ok(EffectOutcome::Suspended)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_placement_appends_to_the_owners_library() {
        let decks = Some(vec![
            vec!["forest".to_string(); 12],
            vec!["forest".to_string(); 12],
        ]);
        let mut engine = GameEngine::new(8906, &[0, 1], 20, decks, true).expect("new game");
        let target = engine.state.players[1]
            .library
            .front()
            .copied()
            .expect("card in library");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            target,
            Zone::Battlefield,
            Some(1),
        )
        .expect("put target on battlefield");

        let mut events = Vec::new();
        move_permanent_to_owners_library(
            &mut engine,
            &mut events,
            target,
            LibraryPlacement::Bottom,
            "test effect",
        )
        .expect("put target on bottom");

        assert_eq!(
            engine.state.players[1].library.back().copied(),
            Some(target)
        );
        assert_eq!(
            engine.state.objects.get(&target).expect("target").zone,
            Zone::Library
        );
    }
}
