use super::*;

pub(super) fn counter_target_spell(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CounterTargetSpell {
        unless_controller_pays,
        ..
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        if let Some(generic_mana_cost) = unless_controller_pays {
            let Some(target) = engine
                .state
                .stack
                .iter()
                .find(|item| item.id == tid)
                .cloned()
            else {
                return Ok(EffectOutcome::Continue);
            };
            let deciding_player = target.controller;
            let prompt = format!(
                "Pay {{{generic_mana_cost}}} to prevent {spell_label} from countering this spell?"
            );
            let payment_currently_legal =
                engine.can_pay_generic_mana(deciding_player, generic_mana_cost);
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                    rv1::ResolutionChoiceRequired {
                        deciding_player_id: deciding_player,
                        source_object_id: cx.top.id,
                        prompt_text: prompt.clone(),
                        choice_kind: custom::ChoiceKind::ManaPayment as i32,
                        candidate_object_ids: Vec::new(),
                        candidate_card_ids: Vec::new(),
                        min: 0,
                        max: 0,
                        ordered: false,
                        candidate_names: Vec::new(),
                        candidate_server_card_ids: Vec::new(),
                        candidate_selectable: Vec::new(),
                        resolution_branches: Vec::new(),
                        mana_cost: String::new(),
                        unique_names: false,
                        generic_mana_cost,
                        payment_currently_legal,
                        reveal_audience: 0,
                        revealed_zone_owner_player_id: None,
                    },
                )),
            });
            engine.state.pending_resolution = Some(PendingResolution {
                deciding_player,
                presentation: PendingResolutionPresentation {
                    source_object_id: cx.top.id,
                    candidates: Vec::new(),
                    min: 0,
                    max: 0,
                    ordered: false,
                    prompt,
                    choice_kind: custom::ChoiceKind::ManaPayment,
                    unique_names: false,
                },
                continuation: ResolutionContinuation::ManaPayment {
                    stack: ParkedStackResolution::new(cx.top.clone()),
                    payment: PendingManaPayment {
                        target_spell_id: tid,
                        generic_mana_cost,
                        mana_cost: ManaCost::default(),
                        undo_history_start: engine.state.undoable_mana_abilities.len(),
                    },
                },
            });
            // The payment is part of this spell's resolution (CR 608.2g). Keep the resolving
            // counter as the top stack item for public/reconnect state until the player pays or
            // declines; the blocking choice prevents it from resolving a second time.
            if !engine.state.stack.iter().any(|item| item.id == cx.top.id) {
                engine.state.stack.push(cx.top.clone());
            }
            return Ok(EffectOutcome::Suspended);
        }
        counter_stack_spell(engine, tid, spell_label, events)?;
    }

    Ok(EffectOutcome::Continue)
}

pub(super) fn counter_triggering_stack_object_unless_pays(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CounterTriggeringStackObjectUnlessPays { cost } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(target) = cx.top.trigger_context.targeting_stack_object else {
        return Ok(EffectOutcome::Continue);
    };
    if !stack_object_ref_present(cx.engine, target) {
        return Ok(EffectOutcome::Continue);
    }
    let deciding_player = cx
        .top
        .trigger_context
        .affected_player
        .ok_or(EngineError::Illegal(
            "Ward payer missing from trigger context",
        ))?;
    let ward_text = cx.top.ability_text.as_deref().unwrap_or("Ward").to_string();
    let prompt = match &cost {
        ResolutionCost::Mana(mana) => format!("Pay {mana} for {ward_text}?"),
        ResolutionCost::DiscardCard { .. } => {
            format!("Discard a matching card to pay for {ward_text}, or decline.")
        }
        ResolutionCost::None | ResolutionCost::SacrificePermanent { .. } => {
            return Err(EngineError::Illegal("unsupported Ward cost"));
        }
    };

    let (presentation, stage, event) = match cost {
        ResolutionCost::Mana(mana_cost) => {
            // Pure generic resolution costs use the established staged-pip transaction: pool
            // clicks and newly produced mana reduce the remainder, the last pip auto-submits, and
            // Decline rewinds payment-time mana abilities. Keeping Ward {2} in `mana_cost` would
            // bypass that flow and require an explicit Pay action.
            let generic_mana_cost = mana_cost
                .pips
                .iter()
                .try_fold(0u32, |total, pip| match pip {
                    ManaSymbol::Generic(amount) => total.checked_add(*amount),
                    _ => None,
                });
            let (generic_mana_cost, payment_mana_cost) = match generic_mana_cost {
                Some(amount) => (amount, ManaCost::default()),
                None => (0, mana_cost.clone()),
            };
            let payment = PendingManaPayment {
                target_spell_id: target.object_id,
                generic_mana_cost,
                mana_cost: payment_mana_cost.clone(),
                undo_history_start: cx.engine.state.undoable_mana_abilities.len(),
            };
            let presentation = PendingResolutionPresentation {
                source_object_id: cx.top.id,
                candidates: Vec::new(),
                min: 0,
                max: 0,
                ordered: false,
                prompt: prompt.clone(),
                choice_kind: custom::ChoiceKind::ManaPayment,
                unique_names: false,
            };
            let event = rv1::ResolutionChoiceRequired {
                deciding_player_id: deciding_player,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: custom::ChoiceKind::ManaPayment as i32,
                candidate_object_ids: Vec::new(),
                candidate_card_ids: Vec::new(),
                min: 0,
                max: 0,
                ordered: false,
                candidate_names: Vec::new(),
                candidate_server_card_ids: Vec::new(),
                candidate_selectable: Vec::new(),
                resolution_branches: Vec::new(),
                mana_cost: payment_mana_cost.to_string(),
                unique_names: false,
                generic_mana_cost,
                payment_currently_legal: if payment_mana_cost.is_empty() {
                    cx.engine
                        .can_pay_generic_mana(deciding_player, generic_mana_cost)
                } else {
                    cx.engine
                        .can_pay_resolution_mana(deciding_player, &payment_mana_cost)
                },
                reveal_audience: 0,
                revealed_zone_owner_player_id: None,
            };
            (presentation, PendingWardPaymentStage::Mana(payment), event)
        }
        ResolutionCost::DiscardCard { filter } => {
            let cost = ResolutionCost::DiscardCard { filter };
            let candidates = cx.engine.resolution_cost_candidates(deciding_player, &cost);
            if candidates.is_empty() {
                counter_stack_object_ref(cx.engine, target, &ward_text, cx.events)?;
                return Ok(EffectOutcome::Continue);
            }
            let candidate_generations = candidates
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
                .collect();
            let candidate_card_ids = candidates
                .iter()
                .map(|oid| {
                    cx.engine
                        .state
                        .objects
                        .get(oid)
                        .map(|object| object.card_id.clone())
                        .unwrap_or_default()
                })
                .collect();
            let candidate_names = candidates
                .iter()
                .map(|oid| object_display_name(&cx.engine.state, cx.engine.registry, *oid))
                .collect();
            let presentation = PendingResolutionPresentation {
                source_object_id: cx.top.id,
                candidates: candidates.clone(),
                min: 0,
                max: 1,
                ordered: false,
                prompt: prompt.clone(),
                choice_kind: custom::ChoiceKind::HandCards,
                unique_names: false,
            };
            let event = rv1::ResolutionChoiceRequired {
                deciding_player_id: deciding_player,
                source_object_id: cx.top.id,
                prompt_text: prompt.clone(),
                choice_kind: custom::ChoiceKind::HandCards as i32,
                candidate_object_ids: candidates,
                candidate_card_ids,
                min: 0,
                max: 1,
                ordered: false,
                candidate_names,
                candidate_server_card_ids: Vec::new(),
                candidate_selectable: Vec::new(),
                resolution_branches: Vec::new(),
                mana_cost: String::new(),
                unique_names: false,
                generic_mana_cost: 0,
                payment_currently_legal: false,
                reveal_audience: 0,
                revealed_zone_owner_player_id: None,
            };
            (
                presentation,
                PendingWardPaymentStage::Discard {
                    candidate_generations,
                },
                event,
            )
        }
        ResolutionCost::None | ResolutionCost::SacrificePermanent { .. } => unreachable!(),
    };

    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(event)),
    });
    cx.events.push(ev_log(format!(
        "P{deciding_player} must decide whether to pay {ward_text}."
    )));
    cx.engine.state.pending_resolution = Some(PendingResolution {
        deciding_player,
        presentation,
        continuation: ResolutionContinuation::WardPayment {
            stack: ParkedStackResolution::new(cx.top.clone()),
            ward: PendingWardPayment { target, stage },
        },
    });
    if !cx
        .engine
        .state
        .stack
        .iter()
        .any(|item| item.id == cx.top.id)
    {
        cx.engine.state.stack.push(cx.top.clone());
    }
    Ok(EffectOutcome::Suspended)
}

pub(crate) fn stack_object_ref_present(engine: &GameEngine, target: StackObjectRef) -> bool {
    if !engine
        .state
        .stack
        .iter()
        .any(|item| item.id == target.object_id)
    {
        return false;
    }
    target.zone_change_generation.is_none_or(|generation| {
        engine
            .state
            .objects
            .get(&target.object_id)
            .is_some_and(|object| object.zone == Zone::Stack)
            && engine
                .state
                .zone_change_generation
                .get(&target.object_id)
                .copied()
                .unwrap_or(0)
                == generation
    })
}

pub(crate) fn counter_stack_object_ref(
    engine: &mut GameEngine,
    target: StackObjectRef,
    counter_label: &str,
    events: &mut Vec<rv1::RuledEvent>,
) -> Result<(), EngineError> {
    if !stack_object_ref_present(engine, target) {
        return Ok(());
    }
    counter_stack_spell(engine, target.object_id, counter_label, events)
}

pub(crate) fn counter_stack_spell(
    engine: &mut GameEngine,
    target_id: ObjectId,
    counter_label: &str,
    events: &mut Vec<rv1::RuledEvent>,
) -> Result<(), EngineError> {
    let Some(pos) = engine
        .state
        .stack
        .iter()
        .position(|item| item.id == target_id)
    else {
        return Ok(());
    };
    let st = engine.state.stack.remove(pos);
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::StackObjectCountered(
            rv1::StackObjectCountered {
                object_id: target_id,
            },
        )),
    });
    let tgt = engine
        .registry
        .get(&st.card_id)
        .map(|definition| definition.name.as_str())
        .unwrap_or("spell");
    if st.ability_text.is_none() && !st.is_copy {
        let owner = engine.state.objects.get(&st.id).map(|object| object.owner);
        let destination = if st.flashback {
            Zone::Exile
        } else {
            Zone::Graveyard
        };
        move_object_to_zone(&mut engine.state, engine.registry, st.id, destination, None)?;
        if let Some(owner) = owner {
            events.push(permanent_moved_event(
                &engine.state,
                st.id,
                owner,
                if st.flashback {
                    rv1::permanent_moved::Destination::Exile
                } else {
                    rv1::permanent_moved::Destination::Graveyard
                },
            ));
        }
    }
    events.push(ev_log(format!("{counter_label} counters {tgt}")));
    Ok(())
}

pub(super) fn copy_target_spell(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CopyTargetSpell { count, .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    // CR 707.10: create `count` copies of the target spell on the stack, each
    // controlled by this spell's controller. The copy is not cast (no mana/cast
    // triggers) and ceases to exist after resolving (handled by `is_copy`). It uses
    // the original's chosen X and face. CR 707.10c: the copy's controller may choose
    // new targets for each copy; if the copied spell has targets, we prompt before
    // placing the copy on the stack. Only the first copy that needs targets is
    // handled via pending_resolution (count=1 covers Twincast/Fork/Reverberate).
    if let Some(&tid) = targets.first() {
        if let Some(src) = engine.state.stack.iter().find(|s| s.id == tid).cloned() {
            let copied_name = engine
                .registry
                .get(&src.card_id)
                .and_then(|d| d.face(src.face_index))
                .map(|f| f.name.to_string())
                .unwrap_or_else(|| src.card_id.clone());
            let src_effects = engine
                .registry
                .get(&src.card_id)
                .and_then(|d| d.face(src.face_index))
                .map(|f| f.spell_effect.to_vec())
                .unwrap_or_default();
            let target_count = if src.chosen_modes.is_empty() {
                src.targets.len()
            } else {
                src.chosen_modes.iter().map(|mode| mode.targets.len()).sum()
            };
            let needs_target_choice = target_count > 0;
            let chosen_mode_indices: Vec<u32> = src
                .chosen_modes
                .iter()
                .map(|mode| mode.mode_index as u32)
                .collect();
            let chosen_mode_labels: Vec<String> = engine
                .registry
                .get(&src.card_id)
                .and_then(|definition| definition.face(src.face_index))
                .and_then(|face| face.modal_spell.as_ref())
                .map(|modal| {
                    src.chosen_modes
                        .iter()
                        .filter_map(|chosen| modal.modes.get(chosen.mode_index))
                        .map(|mode| mode.label.clone())
                        .collect()
                })
                .unwrap_or_default();
            for copy_num in 0..count {
                let copy_id = engine.state.next_object_id;
                engine.state.next_object_id += 1;
                let copy_template = StackItem {
                    id: copy_id,
                    controller,
                    card_id: src.card_id.clone(),
                    targets: src.targets.clone(),
                    ability_text: None,
                    source_permanent_id: None,
                    source_zone_change: 0,
                    source_face_change: 0,
                    ability_index: None,
                    activated_ability: None,
                    triggered_ability: None,
                    is_triggered: false,
                    is_copy: true,
                    chosen_x: src.chosen_x,
                    face_index: src.face_index,
                    chosen_modes: src.chosen_modes.clone(),
                    resolution_branch_choices: Default::default(),
                    // CR 707.2: the copy has the original's characteristics and choices. `None`
                    // for every spell today, but copying inherits it rather than dropping it.
                    trigger_context: src.trigger_context,
                    flashback: false,
                };
                // CR 707.10c: prompt for new targets on the first copy; push any
                // additional copies immediately with the original targets.
                if needs_target_choice && copy_num == 0 {
                    let mut candidates = Vec::new();
                    let candidate_effect_groups: Vec<Vec<SpellEffectKind>> =
                        if src.chosen_modes.is_empty() {
                            vec![src_effects.clone()]
                        } else {
                            src.chosen_modes
                                .iter()
                                .filter_map(|chosen| {
                                    engine
                                        .registry
                                        .get(&src.card_id)
                                        .and_then(|definition| definition.face(src.face_index))
                                        .and_then(|face| face.modal_spell.as_ref())
                                        .and_then(|modal| modal.modes.get(chosen.mode_index))
                                        .map(|mode| mode.effects.clone())
                                })
                                .collect()
                        };
                    for effects in candidate_effect_groups {
                        let sp = compute_spell_targets(
                            engine,
                            controller,
                            TargetSourceIdentity::for_stack_item(engine, &copy_template),
                            &effects,
                            None,
                        );
                        for group in sp.groups {
                            candidates.extend(group.valid_permanent_ids);
                            candidates.extend(group.valid_stack_ids);
                            candidates.extend(group.valid_graveyard_ids);
                            for p in &engine.state.players {
                                if (group.can_target_self && p.id == controller)
                                    || (group.can_target_opponent
                                        && engine.state.are_opponents(p.id, controller))
                                {
                                    candidates.push(p.id as ObjectId);
                                }
                            }
                        }
                    }
                    candidates.sort_unstable();
                    candidates.dedup();
                    // CR 707.10c: may keep original targets even if now illegal.
                    for target in &src.targets {
                        let ot = target.object_id;
                        if !candidates.contains(&ot) {
                            candidates.push(ot);
                        }
                    }
                    let candidate_card_ids: Vec<String> = candidates
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
                    let candidate_names: Vec<String> = candidates
                        .iter()
                        .map(|&oid| {
                            if engine.state.player_idx(oid as i32).is_some() {
                                format!("Player {oid}")
                            } else {
                                engine
                                    .state
                                    .objects
                                    .get(&oid)
                                    .and_then(|o| engine.registry.get(&o.card_id))
                                    .map(|d| d.name.clone())
                                    .unwrap_or_else(|| format!("[object {oid}]"))
                            }
                        })
                        .collect();
                    let prompt = format!("Choose new targets for {copied_name} (copy)");
                    events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                            rv1::ResolutionChoiceRequired {
                                deciding_player_id: controller,
                                source_object_id: copy_id,
                                prompt_text: prompt.clone(),
                                // TargetObjects: client uses click-to-target
                                // instead of a list dialog.
                                choice_kind: custom::ChoiceKind::TargetObjects as i32,
                                candidate_object_ids: candidates.clone(),
                                candidate_card_ids,
                                candidate_names,
                                min: target_count as u32,
                                max: target_count as u32,
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
                            },
                        )),
                    });
                    events.push(ev_log(prompt.clone()));
                    engine.state.pending_resolution = Some(PendingResolution {
                        deciding_player: controller,
                        presentation: PendingResolutionPresentation {
                            source_object_id: copy_id,
                            candidates,
                            min: 1,
                            max: 1,
                            ordered: false,
                            prompt,
                            choice_kind: custom::ChoiceKind::TargetObjects,
                            unique_names: false,
                        },
                        continuation: ResolutionContinuation::CopyTargets {
                            stack: ParkedStackResolution::new(copy_template),
                            copy_source_object_id: src.id,
                        },
                    });
                    // Copy will be pushed to the stack after target is submitted.
                } else {
                    engine.state.stack.push(copy_template);
                    events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                            object_id: copy_id,
                            description: copied_name.clone(),
                            targets: src
                                .targets
                                .iter()
                                .map(|target| rv1::TargetRef {
                                    object_id: target.object_id,
                                    damage_amount: target.damage_amount,
                                    group_index: target.group_index,
                                    kind: target.kind,
                                })
                                .collect(),
                            ability_annotation: "(copy)".to_string(),
                            card_id: src.card_id.clone(),
                            is_copy: true,
                            is_triggered: false,
                            copy_source_object_id: src.id,
                            chosen_mode_indices: chosen_mode_indices.clone(),
                            chosen_mode_labels: chosen_mode_labels.clone(),
                        })),
                    });
                    events.push(ev_log(format!(
                        "{spell_label} copies {copied_name} (P{controller})"
                    )));
                    engine.fire_triggers(&[GameEvent::TargetsChosen {
                        controller,
                        source: TargetingSourceKind::SpellCopy,
                        stack_object: StackObjectRef {
                            object_id: copy_id,
                            zone_change_generation: None,
                        },
                        targets: src.targets.iter().map(|target| target.object_id).collect(),
                    }]);
                }
            }
        }
    }

    Ok(EffectOutcome::Continue)
}
