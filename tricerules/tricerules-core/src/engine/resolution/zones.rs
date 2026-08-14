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
    let idx = engine.state.player_idx(drawer).unwrap();
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

    Ok(EffectOutcome::Continue)
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

pub(super) fn return_target_to_hand(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let (SpellEffectKind::ReturnTargetCreatureToHand
    | SpellEffectKind::ReturnTargetPermanentToHand) = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
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

pub(super) fn discard_cards(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::DiscardCards {
        count,
        target: _,
        random,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let top = cx.top;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        if let Some(pi) = engine.state.player_idx(tid as i32) {
            let pid = engine.state.players[pi].id;
            if random {
                // CR 701.7a "at random": engine chooses using a seeded RNG so
                // replays reconstruct the same discard (Hymn to Tourach).
                let hand = engine.state.players[pi].hand.clone();
                let discard_count = (count as usize).min(hand.len());
                if discard_count == 0 {
                    events.push(ev_log(format!(
                        "P{pid} has no cards to discard ({spell_label})."
                    )));
                } else {
                    let mix = engine
                        .state
                        .command_index
                        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
                    let mut rng = rand::rngs::StdRng::seed_from_u64(mix);
                    let mut shuffled = hand;
                    shuffled.shuffle(&mut rng);
                    let to_discard: Vec<ObjectId> =
                        shuffled.into_iter().take(discard_count).collect();
                    for oid in &to_discard {
                        let card_name = object_display_name(&engine.state, engine.registry, *oid);
                        move_object_to_zone(
                            &mut engine.state,
                            engine.registry,
                            *oid,
                            Zone::Graveyard,
                            None,
                        )?;
                        events.push(permanent_moved_event(
                            &engine.state,
                            *oid,
                            pid,
                            rv1::permanent_moved::Destination::Graveyard,
                        ));
                        events.push(ev_log(format!(
                            "P{pid} discards {card_name} ({spell_label})."
                        )));
                    }
                }
            } else {
                // Caster-chooses: emit ResolutionChoiceRequired so the caster
                // sees the target's revealed hand and picks which cards to discard
                // (Coercion, Thoughtseize). choice_kind 1 = RevealedCards (public).
                let hand: Vec<ObjectId> = engine.state.players[pi].hand.clone();
                if hand.is_empty() {
                    events.push(ev_log(format!(
                        "P{pid} has no cards to discard ({spell_label})."
                    )));
                } else {
                    let n = (hand.len() as u32).min(count);
                    let candidate_card_ids: Vec<String> = hand
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
                    let prompt =
                        format!("P{controller}: choose {n} card(s) to discard from P{pid}'s hand.");
                    events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                            rv1::ResolutionChoiceRequired {
                                deciding_player_id: controller,
                                source_object_id: top.id,
                                prompt_text: prompt.clone(),
                                // Private: "look at target player's hand" reveals
                                // it only to the caster, not the table (CR 701.7).
                                choice_kind: custom::ChoiceKind::OpponentHand as i32,
                                candidate_object_ids: hand.clone(),
                                candidate_card_ids,
                                candidate_names,
                                min: n,
                                max: n,
                                ordered: false,
                                unique_names: false,
                                candidate_server_card_ids: Vec::new(),
                                generic_mana_cost: 0,
                                payment_currently_legal: false,
                            },
                        )),
                    });
                    events.push(ev_log(prompt.clone()));
                    engine.state.pending_resolution = Some(PendingResolution {
                        item: top.clone(),
                        custom_key: "__discard_chosen".to_string(),
                        step: 0,
                        scratch: vec![],
                        deciding_player: controller,
                        candidates: hand,
                        min: n,
                        max: n,
                        ordered: false,
                        unique_names: false,
                        mana_payment: None,
                        prompt,
                        choice_kind: custom::ChoiceKind::OpponentHand,
                        copy_source_object_id: 0,
                        search_destination: SearchDestination::Hand,
                        search_shuffle: false,
                        search_reveal: false,
                        // Stamped by `run_effect_list` once it sees the `Suspended` below.
                        resume_effect_index: None,
                    });
                    return Ok(EffectOutcome::Suspended);
                }
            }
        }
    }

    Ok(EffectOutcome::Continue)
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
    let recipients = player_recipients(
        &cx.engine.state,
        cx.controller,
        cx.affected_player,
        trigger_object_controller(cx.engine, cx.top),
        who,
    );
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
                            generic_mana_cost: 0,
                            payment_currently_legal: false,
                        },
                    )),
                });
                events.push(ev_log(prompt.clone()));
                engine.state.pending_resolution = Some(PendingResolution {
                    item: top.clone(),
                    custom_key: "__sacrifice_chosen".to_string(),
                    step: 0,
                    scratch: vec![],
                    deciding_player: pid,
                    candidates: qualifying,
                    min: 1,
                    max: 1,
                    ordered: false,
                    unique_names: false,
                    mana_payment: None,
                    prompt,
                    choice_kind: custom::ChoiceKind::Revealed,
                    copy_source_object_id: 0,
                    search_destination: SearchDestination::default(),
                    search_shuffle: false,
                    search_reveal: false,
                    // Stamped by `run_effect_list` once it sees the `Suspended` below.
                    resume_effect_index: None,
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
                        chosen_x: 0,
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

pub(super) fn return_ability_source_from_graveyard(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ReturnAbilitySourceFromGraveyard { tapped, controller } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(source_id) = cx.top.source_permanent_id else {
        return Ok(EffectOutcome::Continue);
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
    if object.zone != Zone::Graveyard
        || current_generation != cx.top.source_zone_change.saturating_add(1)
    {
        return Ok(EffectOutcome::Continue);
    }

    let owner = object.owner;
    let destination_controller = match controller {
        ReturnController::Owner => owner,
        ReturnController::AbilityController => cx.controller,
    };
    let object_label = object_display_name(&cx.engine.state, cx.engine.registry, source_id);
    match cx.engine.begin_battlefield_entry(
        cx.top.clone(),
        BattlefieldEntryEvent {
            object_id: source_id,
            deciding_player: owner,
            destination_controller,
            face_index: 0,
            chosen_x: 0,
            tapped,
            entry_counters: BTreeMap::new(),
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
        item: top.clone(),
        custom_key: "__scry".to_string(),
        step: 0,
        // The looked-at set: `finish_scry` needs it to work out which cards were *not* sent to
        // the bottom, since the choice only names the ones that were.
        scratch: candidates.clone(),
        deciding_player: controller,
        candidates,
        min: 0,
        max: n,
        ordered: true,
        unique_names: false,
        mana_payment: None,
        prompt,
        choice_kind: custom::ChoiceKind::LibraryTop,
        copy_source_object_id: 0,
        search_destination: SearchDestination::default(),
        search_shuffle: false,
        search_reveal: false,
        // Stamped by `run_effect_list` once it sees the `Suspended` below — this is what makes
        // `[Scry, Draw]` (Preordain, Opt) draw the card after the scry decision.
        resume_effect_index: None,
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
    let min = if candidates.is_empty() { 0u32 } else { 1u32 };
    let prompt = match &filter {
        None => format!("P{controller}: search your library for a card."),
        Some(f) => format!(
            "P{controller}: search your library for a {} card.",
            card_type_filter_desc(f)
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
                generic_mana_cost: 0,
                payment_currently_legal: false,
            },
        )),
    });
    events.push(ev_log(prompt.clone()));
    engine.state.pending_resolution = Some(PendingResolution {
        item: top.clone(),
        custom_key: "__search_library".to_string(),
        step: 0,
        scratch: vec![],
        deciding_player: controller,
        candidates,
        min,
        max: 1,
        ordered: false,
        unique_names: false,
        mana_payment: None,
        prompt,
        choice_kind: custom::ChoiceKind::LibrarySearch,
        copy_source_object_id: 0,
        search_destination: destination,
        search_shuffle: shuffle,
        search_reveal: reveal,
        // Stamped by `run_effect_list` once it sees the `Suspended` below.
        resume_effect_index: None,
    });
    // Resolution is now parked; the "resolves." log is emitted by finish_library_search.
    Ok(EffectOutcome::Suspended)
}
