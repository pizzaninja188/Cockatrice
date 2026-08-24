use super::super::events::ev_log_private;
use super::candidate_identities;
use super::*;

pub(super) fn draw(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Draw { who, count } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let drawers = player_recipients(cx, who);
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let top = cx.top;
    let spell_label = cx.spell_label;

    // Blue Sun's Zenith / Braingeyser: `count` may be the cast-time X.
    let count = engine.resolve_amount(
        &count,
        AmountContext::for_stack_item(top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    for drawer in drawers {
        draw_cards_for_player(engine, events, drawer, count, spell_label)?;
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn exile_top_with_play_permission(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ExileTopWithPlayPermission { player } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let recipients = player_recipients(cx, player);
    let spell_label = cx.spell_label.to_string();
    for recipient in recipients {
        let player_index = cx
            .engine
            .state
            .player_idx(recipient)
            .ok_or(EngineError::UnknownPlayer(recipient))?;
        let Some(object_id) = cx.engine.state.players[player_index]
            .library
            .front()
            .copied()
        else {
            continue;
        };
        let card_name = object_display_name(&cx.engine.state, cx.engine.registry, object_id);
        move_object_to_zone(
            &mut cx.engine.state,
            cx.engine.registry,
            object_id,
            Zone::Exile,
            None,
        )?;
        cx.events.push(permanent_moved_event_with_library_position(
            &cx.engine.state,
            object_id,
            recipient,
            rv1::permanent_moved::Destination::Exile,
            0,
        ));
        cx.engine.grant_exile_play_permission(
            recipient,
            object_id,
            &spell_label,
            ExilePlayPermissionScope::PlayCard,
            true,
        )?;
        cx.events.push(ev_log(format!(
            "P{recipient} exiles {card_name} and may play it until the end of their next turn ({spell_label})."
        )));
    }
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
        engine.fire_card_drawn(drawer);
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

pub(in crate::engine) fn move_permanent_to_owners_library(
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
        LibraryPlacement::OwnerChoiceTopOrBottom => {
            return Err(EngineError::Illegal(
                "owner library placement choice must be resolved before movement",
            ));
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
        if placement == LibraryPlacement::OwnerChoiceTopOrBottom {
            let Some(object) = cx.engine.state.objects.get(&tid) else {
                return Ok(EffectOutcome::Continue);
            };
            let owner = object.owner;
            let generation = cx
                .engine
                .state
                .zone_change_generation
                .get(&tid)
                .copied()
                .unwrap_or(0);
            let prompt = format!(
                "P{owner}: put {} on the top or bottom of its owner's library.",
                object_display_name(&cx.engine.state, cx.engine.registry, tid)
            );
            cx.events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                    rv1::ResolutionChoiceRequired {
                        deciding_player_id: owner,
                        source_object_id: cx.top.id,
                        prompt_text: prompt.clone(),
                        choice_kind: custom::ChoiceKind::ResolutionBranch as i32,
                        candidate_object_ids: Vec::new(),
                        candidate_card_ids: Vec::new(),
                        // Exactly one placement branch is mandatory. These cardinalities also
                        // drive the ruled client's Decline affordance; 0/0 would falsely present
                        // this CR 608 choice as optional even though submission rejects decline.
                        min: 1,
                        max: 1,
                        ordered: false,
                        candidate_names: Vec::new(),
                        candidate_server_card_ids: Vec::new(),
                        unique_names: false,
                        generic_mana_cost: 0,
                        payment_currently_legal: false,
                        resolution_branches: vec![
                            rv1::ResolutionBranchOption {
                                branch_index: 0,
                                label: "Top".into(),
                                cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
                                cost_text: String::new(),
                                selectable: true,
                                search_zones: Vec::new(),
                            },
                            rv1::ResolutionBranchOption {
                                branch_index: 1,
                                label: "Bottom".into(),
                                cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
                                cost_text: String::new(),
                                selectable: true,
                                search_zones: Vec::new(),
                            },
                        ],
                        mana_cost: String::new(),
                        candidate_selectable: Vec::new(),
                        reveal_audience: 0,
                        revealed_zone_owner_player_id: None,
                        candidate_source_zones: Vec::new(),
                    },
                )),
            });
            cx.events.push(ev_log(prompt.clone()));
            cx.engine.state.pending_resolution = Some(PendingResolution {
                deciding_player: owner,
                presentation: PendingResolutionPresentation {
                    source_object_id: cx.top.id,
                    candidates: Vec::new(),
                    min: 1,
                    max: 1,
                    ordered: false,
                    prompt,
                    choice_kind: custom::ChoiceKind::ResolutionBranch,
                    unique_names: false,
                },
                continuation: ResolutionContinuation::OwnerLibraryPlacement {
                    stack: ParkedStackResolution::new(cx.top.clone()),
                    object_id: tid,
                    owner,
                    zone_change_generation: generation,
                    spell_label: cx.spell_label.to_string(),
                },
            });
            return Ok(EffectOutcome::Suspended);
        }
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
        visibility,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(&target) = cx.targets.first() else {
        return Ok(EffectOutcome::Continue);
    };
    choose_hand_cards_for_player(
        cx,
        target as PlayerId,
        HandCardChoiceSpec {
            count,
            chooser,
            card_filter: card_filter.as_ref(),
            optional,
            visibility,
            draw_after: 0,
            action: HandCardAction::Discard,
        },
    )
}

pub(super) fn exile_cards_from_hand(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ExileCardsFromHand {
        count,
        target: _,
        chooser,
        card_filter,
        optional,
        visibility,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(&target) = cx.targets.first() else {
        return Ok(EffectOutcome::Continue);
    };
    choose_hand_cards_for_player(
        cx,
        target as PlayerId,
        HandCardChoiceSpec {
            count,
            chooser,
            card_filter: card_filter.as_ref(),
            optional,
            visibility,
            draw_after: 0,
            action: HandCardAction::Exile,
        },
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
            choose_hand_cards_for_player(
                cx,
                player,
                HandCardChoiceSpec {
                    count: discard_count,
                    chooser: DiscardChooser::AffectedPlayer,
                    card_filter: None,
                    optional: false,
                    visibility: HandChoiceVisibility::PrivateLook,
                    draw_after: 0,
                    action: HandCardAction::Discard,
                },
            )
        }
        DrawDiscardOrder::DiscardThenDraw => choose_hand_cards_for_player(
            cx,
            player,
            HandCardChoiceSpec {
                count: discard_count,
                chooser: DiscardChooser::AffectedPlayer,
                card_filter: None,
                optional,
                visibility: HandChoiceVisibility::PrivateLook,
                draw_after: draw_count,
                action: HandCardAction::Discard,
            },
        ),
    }
}

struct HandCardChoiceSpec<'a> {
    count: u32,
    chooser: DiscardChooser,
    card_filter: Option<&'a CardTypeFilter>,
    optional: bool,
    visibility: HandChoiceVisibility,
    draw_after: u32,
    action: HandCardAction,
}

fn choose_hand_cards_for_player(
    cx: &mut EffectCx<'_>,
    affected_player: PlayerId,
    spec: HandCardChoiceSpec<'_>,
) -> Result<EffectOutcome, EngineError> {
    let HandCardChoiceSpec {
        count,
        chooser,
        card_filter,
        optional,
        visibility,
        draw_after,
        action,
    } = spec;
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
        let chosen_count = (count as usize).min(eligible.len());
        if chosen_count == 0 {
            events.push(ev_log(format!(
                "P{affected_player} has no eligible cards to {} ({spell_label}).",
                hand_action_verb(action)
            )));
        } else {
            let mut shuffled = eligible;
            shuffle_object_ids_for_current_command(&engine.state, affected_player, &mut shuffled);
            for oid in shuffled.into_iter().take(chosen_count) {
                perform_hand_card_action(
                    engine,
                    events,
                    affected_player,
                    oid,
                    action,
                    spell_label,
                )?;
            }
        }
        return Ok(EffectOutcome::Continue);
    }

    if eligible.is_empty() {
        events.push(ev_log(format!(
            "P{affected_player} has no eligible cards to {} ({spell_label}).",
            hand_action_verb(action)
        )));
        if action == HandCardAction::Discard && draw_after > 0 && !optional {
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
    let verb = hand_action_verb(action);
    let prompt = if optional {
        format!("P{deciding_player}: you may choose a card for P{affected_player} to {verb}.")
    } else {
        format!("P{deciding_player}: choose {n} card(s) for P{affected_player} to {verb}.")
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
                reveal_audience: if visibility == HandChoiceVisibility::PublicReveal {
                    rv1::ResolutionRevealAudience::AllParticipants as i32
                } else {
                    rv1::ResolutionRevealAudience::None as i32
                },
                revealed_zone_owner_player_id: (visibility == HandChoiceVisibility::PublicReveal)
                    .then_some(affected_player),
                candidate_source_zones: Vec::new(),
            },
        )),
    });
    events.push(ev_log(prompt.clone()));
    engine.state.pending_resolution = Some(PendingResolution {
        deciding_player,
        presentation: PendingResolutionPresentation {
            source_object_id: top.id,
            candidates: eligible.clone(),
            min,
            max: n,
            ordered: false,
            unique_names: false,
            prompt,
            choice_kind,
        },
        continuation: ResolutionContinuation::HandChoice {
            stack: ParkedStackResolution::new(top.clone()),
            hand_choice: PendingHandChoice {
                affected_player,
                action,
                candidate_generations: eligible
                    .iter()
                    .map(|object_id| {
                        (
                            *object_id,
                            engine
                                .state
                                .zone_change_generation
                                .get(object_id)
                                .copied()
                                .unwrap_or(0),
                        )
                    })
                    .collect(),
                draw_after,
                draw_only_if_discarded: optional,
            },
        },
    });
    Ok(EffectOutcome::Suspended)
}

fn hand_action_verb(action: HandCardAction) -> &'static str {
    match action {
        HandCardAction::Discard => "discard",
        HandCardAction::Exile => "exile",
    }
}

fn perform_discard_action(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    affected_player: PlayerId,
    object_id: ObjectId,
    spell_label: &str,
) -> Result<(), EngineError> {
    let (card_name, moved) = perform_discard(
        &mut engine.state,
        engine.registry,
        affected_player,
        object_id,
    )?;
    events.push(moved);
    events.push(ev_log(format!(
        "P{affected_player} discards {card_name} ({spell_label})."
    )));
    Ok(())
}

fn perform_exile_from_hand(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    affected_player: PlayerId,
    object_id: ObjectId,
    spell_label: &str,
) -> Result<(), EngineError> {
    let card_name = object_display_name(&engine.state, engine.registry, object_id);
    move_object_to_zone(
        &mut engine.state,
        engine.registry,
        object_id,
        Zone::Exile,
        None,
    )?;
    events.push(permanent_moved_event(
        &engine.state,
        object_id,
        affected_player,
        rv1::permanent_moved::Destination::Exile,
    ));
    events.push(ev_log(format!(
        "P{affected_player} exiles {card_name} from their hand ({spell_label})."
    )));
    Ok(())
}

pub(crate) fn perform_hand_card_action(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    affected_player: PlayerId,
    object_id: ObjectId,
    action: HandCardAction,
    spell_label: &str,
) -> Result<(), EngineError> {
    match action {
        HandCardAction::Discard => {
            perform_discard_action(engine, events, affected_player, object_id, spell_label)
        }
        HandCardAction::Exile => {
            perform_exile_from_hand(engine, events, affected_player, object_id, spell_label)
        }
    }
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
                            reveal_audience: 0,
                            revealed_zone_owner_player_id: None,
                            candidate_source_zones: Vec::new(),
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

pub(super) fn choose_graveyard_card(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::ChooseGraveyardCard {
        filter,
        destination,
        optional,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let controller = cx.controller;
    let Some(index) = cx.engine.state.player_idx(controller) else {
        return Ok(EffectOutcome::Continue);
    };
    let candidates: Vec<ObjectId> = cx.engine.state.players[index]
        .graveyard
        .iter()
        .copied()
        .filter(|oid| {
            library_card_matches_filter(&cx.engine.state, cx.engine.registry, *oid, Some(&filter))
        })
        .collect();
    if candidates.is_empty() {
        cx.events.push(ev_log(format!(
            "P{controller} has no matching graveyard card ({}).",
            cx.spell_label
        )));
        return Ok(EffectOutcome::Continue);
    }
    let min = if optional { 0 } else { 1 };
    let prompt = if optional {
        format!("P{controller}: you may choose a matching card from your graveyard.")
    } else {
        format!("P{controller}: choose a matching card from your graveyard.")
    };
    let (candidate_card_ids, candidate_names) = candidate_identities(cx.engine, &candidates);
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: controller,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: custom::ChoiceKind::GraveyardCards as i32,
                candidate_object_ids: candidates.clone(),
                candidate_card_ids,
                min,
                max: 1,
                ordered: false,
                candidate_names,
                candidate_server_card_ids: Vec::new(),
                unique_names: false,
                generic_mana_cost: 0,
                payment_currently_legal: false,
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                candidate_selectable: Vec::new(),
                reveal_audience: 0,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: vec![
                    rv1::ChoiceCandidateSourceZone::Graveyard as i32;
                    candidates.len()
                ],
            },
        )),
    });
    cx.events.push(ev_log(prompt.clone()));
    cx.engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: controller,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates: candidates.clone(),
            min,
            max: 1,
            ordered: false,
            prompt,
            choice_kind: custom::ChoiceKind::GraveyardCards,
            unique_names: false,
        },
        continuation: ResolutionContinuation::GraveyardChoice {
            stack: ParkedStackResolution::new(cx.top.clone()),
            destination,
            candidate_generations: candidates
                .iter()
                .map(|oid| {
                    (
                        *oid,
                        cx.engine
                            .state
                            .zone_change_generation
                            .get(oid)
                            .copied()
                            .unwrap_or(0),
                    )
                })
                .collect(),
            spell_label: cx.spell_label.to_string(),
        },
    });
    Ok(EffectOutcome::Suspended)
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
                GraveyardDestination::Battlefield { .. } => Zone::Battlefield,
            };
            let dest_proto = match destination {
                GraveyardDestination::Hand => rv1::permanent_moved::Destination::Hand,
                GraveyardDestination::Battlefield { .. } => {
                    rv1::permanent_moved::Destination::Battlefield
                }
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
                        cast_cost_receipts: Vec::new(),
                        player_life_snapshot: engine.player_life_snapshot(),
                        tapped: matches!(
                            destination,
                            GraveyardDestination::Battlefield { tapped: true }
                        ),
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
                GraveyardDestination::Battlefield { .. } => "battlefield",
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
            cast_cost_receipts: Vec::new(),
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

/// CR 701.18 scry enters the shared private top-library partition state machine. Scry's selected
/// cohort goes to the library bottom; the retained cohort may require a second ordering choice.
pub(super) fn scry(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Scry { count } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    begin_library_partition(cx, count, 0, None, PendingLibraryPartitionKind::Scry)
}

/// CR 701.25 surveil and nonkeyword bounded looks share the same private choice contract. The
/// selected cohort goes to the graveyard; cards retained on top are ordered in a second step when
/// necessary.
pub(super) fn library_partition(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::LibraryPartition {
        count,
        top_min,
        top_max,
        kind,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let pending_kind = match kind {
        LibraryPartitionKind::Surveil => PendingLibraryPartitionKind::Surveil,
        LibraryPartitionKind::Look => PendingLibraryPartitionKind::Look,
    };
    begin_library_partition(cx, count, top_min, top_max, pending_kind)
}

fn begin_library_partition(
    cx: &mut EffectCx<'_>,
    count: u32,
    top_min: u32,
    top_max: Option<u32>,
    kind: PendingLibraryPartitionKind,
) -> Result<EffectOutcome, EngineError> {
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
        let verb = match kind {
            PendingLibraryPartitionKind::Scry => "scries",
            PendingLibraryPartitionKind::Surveil => "surveils",
            PendingLibraryPartitionKind::Look => "looks at",
        };
        events.push(ev_log(format!(
            "P{controller} {verb} {count} with an empty library ({spell_label})."
        )));
        // CR 701.25d: the surveil event happens after the process is complete even when every
        // action was impossible because the library was empty.
        if kind == PendingLibraryPartitionKind::Surveil {
            engine.fire_triggers(&[GameEvent::Surveilled { player: controller }]);
        }
        return Ok(EffectOutcome::Continue);
    }

    let (candidate_card_ids, candidate_names) = candidate_identities(engine, &candidates);
    let effective_top_min = top_min.min(n);
    let effective_top_max = top_max.unwrap_or(n).min(n);
    let destination_min = n - effective_top_max;
    let destination_max = n - effective_top_min;
    let (prompt, choice_kind, candidate_selectable) = match kind {
        PendingLibraryPartitionKind::Scry => (
            format!(
                "Scry {n}: click any number of cards to put on the bottom of your library — they \
                 go down in the order you click them, so the last one ends up bottom-most. The \
                 rest stay on top."
            ),
            custom::ChoiceKind::LibraryTop,
            Vec::new(),
        ),
        PendingLibraryPartitionKind::Surveil => (
            format!(
                "Surveil {n}: click any number of cards to put into your graveyard — the last one \
                 you click becomes the top card of your graveyard. The rest stay on top of your \
                 library."
            ),
            custom::ChoiceKind::LibraryLook,
            vec![true; n as usize],
        ),
        PendingLibraryPartitionKind::Look => (
            format!(
                "Look at the top {n} cards: click between {destination_min} and {destination_max} \
                 cards to put into your graveyard — the last one you click becomes the top card \
                 of your graveyard. The rest stay on top of your library."
            ),
            custom::ChoiceKind::LibraryLook,
            vec![true; n as usize],
        ),
    };
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: controller,
                source_object_id: top.id,
                prompt_text: prompt.clone(),
                choice_kind: choice_kind as i32,
                candidate_object_ids: candidates.clone(),
                candidate_card_ids,
                candidate_names: candidate_names.clone(),
                min: destination_min,
                max: destination_max,
                ordered: true,
                unique_names: false,
                candidate_server_card_ids: Vec::new(),
                candidate_selectable,
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                generic_mana_cost: 0,
                payment_currently_legal: false,
                reveal_audience: 0,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: Vec::new(),
            },
        )),
    });
    let verb = match kind {
        PendingLibraryPartitionKind::Scry => "scries",
        PendingLibraryPartitionKind::Surveil => "surveils",
        PendingLibraryPartitionKind::Look => "looks at",
    };
    // The count is public knowledge; the identities are not.
    events.push(ev_log(format!("P{controller} {verb} {n} ({spell_label}).")));
    events.push(ev_log_private(
        format!("P{controller} looks at {}.", candidate_names.join(", ")),
        controller,
    ));
    engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: controller,
        presentation: PendingResolutionPresentation {
            source_object_id: top.id,
            candidates: candidates.clone(),
            min: destination_min,
            max: destination_max,
            ordered: true,
            unique_names: false,
            prompt,
            choice_kind,
        },
        continuation: ResolutionContinuation::LibraryPartition {
            stack: ParkedStackResolution::new(top.clone()),
            looked_at: candidates,
            stage: PendingLibraryPartitionStage::ChooseDestination,
            kind,
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
                cast_cost_receipts: Vec::new(),
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
                reveal_audience: 0,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: Vec::new(),
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
        .map(|&oid| library_card_matches_filter(&engine.state, engine.registry, oid, Some(&filter)))
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
                reveal_audience: 0,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: Vec::new(),
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

pub(in crate::engine) fn search_zone_combinations(
    available: &[CardSearchZone],
) -> Vec<Vec<CardSearchZone>> {
    let mut combinations = Vec::new();
    for size in 1..=available.len() {
        for mask in 1usize..(1usize << available.len()) {
            if mask.count_ones() as usize == size {
                combinations.push(
                    available
                        .iter()
                        .enumerate()
                        .filter_map(|(index, zone)| ((mask & (1 << index)) != 0).then_some(*zone))
                        .collect(),
                );
            }
        }
    }
    combinations
}

fn search_zone_label(zone: CardSearchZone) -> &'static str {
    match zone {
        CardSearchZone::Hand => "Hand",
        CardSearchZone::Graveyard => "Graveyard",
        CardSearchZone::Library => "Library",
    }
}

fn search_zone_proto(zone: CardSearchZone) -> i32 {
    match zone {
        CardSearchZone::Hand => rv1::ChoiceCandidateSourceZone::Hand as i32,
        CardSearchZone::Graveyard => rv1::ChoiceCandidateSourceZone::Graveyard as i32,
        CardSearchZone::Library => rv1::ChoiceCandidateSourceZone::Library as i32,
    }
}

pub(in crate::engine) struct ZoneSearchRequest {
    pub count: u32,
    pub filter: Option<ZoneCardFilter>,
    pub zones: Vec<CardSearchZone>,
    pub destination: SearchDestination,
    pub conditional_destination: Option<ConditionalSearchDestination>,
    pub shuffle: bool,
    pub reveal: bool,
}

pub(in crate::engine) fn park_zone_search_choice(
    engine: &mut GameEngine,
    events: &mut Vec<rv1::RuledEvent>,
    top: &StackItem,
    request: ZoneSearchRequest,
) -> Result<(), EngineError> {
    let ZoneSearchRequest {
        count,
        filter,
        zones,
        destination,
        conditional_destination,
        shuffle,
        reveal,
    } = request;
    let controller = top.controller;
    let idx = engine
        .state
        .player_idx(controller)
        .ok_or(EngineError::UnknownPlayer(controller))?;
    let mut candidates = Vec::new();
    let mut candidate_zones = Vec::new();
    for zone in &zones {
        let cohort: Vec<ObjectId> = match zone {
            CardSearchZone::Hand => engine.state.players[idx].hand.clone(),
            CardSearchZone::Graveyard => engine.state.players[idx].graveyard.clone(),
            CardSearchZone::Library => engine.state.players[idx].library.iter().copied().collect(),
        };
        for oid in cohort.into_iter().filter(|oid| {
            library_card_matches_filter(&engine.state, engine.registry, *oid, filter.as_ref())
        }) {
            candidates.push(oid);
            candidate_zones.push(search_zone_proto(*zone));
        }
    }
    let public_graveyard_match = candidates
        .iter()
        .zip(&candidate_zones)
        .any(|(_, zone)| *zone == rv1::ChoiceCandidateSourceZone::Graveyard as i32);
    let min = if public_graveyard_match || (filter.is_none() && !candidates.is_empty()) {
        count.min(candidates.len() as u32)
    } else {
        0
    };
    let zone_names = zones
        .iter()
        .map(|zone| search_zone_label(*zone))
        .collect::<Vec<_>>()
        .join(" / ");
    let prompt = format!("P{controller}: search {zone_names} for up to {count} matching card(s).");
    let (candidate_card_ids, candidate_names) = candidate_identities(engine, &candidates);
    let multi_zone = zones.len() > 1 || zones.first() != Some(&CardSearchZone::Library);
    let choice_kind = if multi_zone {
        custom::ChoiceKind::ZoneSearch
    } else {
        custom::ChoiceKind::LibrarySearch
    };
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                deciding_player_id: controller,
                source_object_id: top.id,
                prompt_text: prompt.clone(),
                choice_kind: choice_kind as i32,
                candidate_object_ids: candidates.clone(),
                candidate_card_ids,
                candidate_names,
                min,
                max: count,
                ordered: false,
                unique_names: false,
                candidate_server_card_ids: Vec::new(),
                candidate_selectable: Vec::new(),
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                generic_mana_cost: 0,
                payment_currently_legal: false,
                reveal_audience: 0,
                revealed_zone_owner_player_id: None,
                candidate_source_zones: if multi_zone {
                    candidate_zones
                } else {
                    Vec::new()
                },
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
            max: count,
            ordered: false,
            unique_names: false,
            prompt,
            choice_kind,
        },
        continuation: ResolutionContinuation::SearchLibrary {
            stack: ParkedStackResolution::new(top.clone()),
            zones,
            destination,
            conditional_destination,
            shuffle,
            reveal,
        },
    });
    Ok(())
}

pub(super) fn search_library(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::SearchLibrary {
        filter,
        count,
        count_by_cast_cost,
        zones,
        destination,
        conditional_destination,
        shuffle,
        reveal,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let count = count_by_cast_cost.map_or(count, |conditional| {
        if cx.top.cast_cost_condition_matches(conditional.condition) {
            conditional.if_selected
        } else {
            conditional.otherwise
        }
    });
    match zones {
        SearchZoneSelection::Fixed(zones) => park_zone_search_choice(
            cx.engine,
            cx.events,
            cx.top,
            ZoneSearchRequest {
                count,
                filter,
                zones,
                destination,
                conditional_destination,
                shuffle,
                reveal,
            },
        )?,
        SearchZoneSelection::PlayerChoice(available_zones) => {
            let combinations = search_zone_combinations(&available_zones);
            let prompt = format!("P{}: choose which zones to search.", cx.controller);
            let branches = combinations
                .iter()
                .enumerate()
                .map(|(index, zones)| rv1::ResolutionBranchOption {
                    branch_index: index as u32,
                    label: zones
                        .iter()
                        .map(|zone| search_zone_label(*zone))
                        .collect::<Vec<_>>()
                        .join(" + "),
                    cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
                    cost_text: String::new(),
                    selectable: true,
                    search_zones: zones.iter().map(|zone| search_zone_proto(*zone)).collect(),
                })
                .collect();
            cx.events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                    rv1::ResolutionChoiceRequired {
                        deciding_player_id: cx.controller,
                        source_object_id: cx.top.id,
                        prompt_text: prompt.clone(),
                        choice_kind: custom::ChoiceKind::ResolutionBranch as i32,
                        candidate_object_ids: Vec::new(),
                        candidate_card_ids: Vec::new(),
                        // The player must choose one nonempty authored zone combination before
                        // the search can begin. Failure to find happens in the following search,
                        // not by declining this scope choice.
                        min: 1,
                        max: 1,
                        ordered: false,
                        candidate_names: Vec::new(),
                        candidate_server_card_ids: Vec::new(),
                        unique_names: false,
                        generic_mana_cost: 0,
                        payment_currently_legal: false,
                        resolution_branches: branches,
                        mana_cost: String::new(),
                        candidate_selectable: Vec::new(),
                        reveal_audience: 0,
                        revealed_zone_owner_player_id: None,
                        candidate_source_zones: Vec::new(),
                    },
                )),
            });
            cx.events.push(ev_log(prompt.clone()));
            cx.engine.state.pending_resolution = Some(PendingResolution {
                deciding_player: cx.controller,
                presentation: PendingResolutionPresentation {
                    source_object_id: cx.top.id,
                    candidates: Vec::new(),
                    min: 1,
                    max: 1,
                    ordered: false,
                    prompt,
                    choice_kind: custom::ChoiceKind::ResolutionBranch,
                    unique_names: false,
                },
                continuation: ResolutionContinuation::SearchZoneScope {
                    stack: ParkedStackResolution::new(cx.top.clone()),
                    count,
                    available_zones,
                    filter,
                    destination,
                    conditional_destination,
                    shuffle,
                    reveal,
                },
            });
        }
    }
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

    #[test]
    fn three_search_zones_produce_all_seven_nonempty_combinations_deterministically() {
        use CardSearchZone::{Graveyard, Hand, Library};
        assert_eq!(
            search_zone_combinations(&[Hand, Graveyard, Library]),
            vec![
                vec![Hand],
                vec![Graveyard],
                vec![Library],
                vec![Hand, Graveyard],
                vec![Hand, Library],
                vec![Graveyard, Library],
                vec![Hand, Graveyard, Library],
            ]
        );
    }
}
