use super::*;

pub(super) fn counter_target_spell(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CounterTargetSpell { .. } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        if let Some(pos) = engine.state.stack.iter().position(|s| s.id == tid) {
            let st = engine.state.stack.remove(pos);
            let tgt = engine
                .registry
                .get(&st.card_id)
                .map(|d| d.name.as_str())
                .unwrap_or("spell");
            // CR 707.10d: a copy of a spell has no backing card — it simply ceases
            // to exist when it leaves the stack. Only a genuinely cast spell has a
            // `GameObject` that moves; CR 701.6a sends that card to its OWNER's
            // graveyard via an explicit PermanentMoved so the C++ relay routes the
            // physical card off the shared stack. Moving a copy would error on the
            // missing object and corrupt the already-popped stack.
            if !st.is_copy {
                let owner = engine.state.objects.get(&st.id).map(|o| o.owner);
                let destination = if st.flashback {
                    Zone::Exile
                } else {
                    Zone::Graveyard
                };
                move_object_to_zone(&mut engine.state, st.id, destination, None)?;
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
            events.push(ev_log(format!("{spell_label} counters {tgt}")));
        }
    }

    Ok(EffectOutcome::Continue)
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
            // Modal copies retain their complete per-mode target groups atomically. The existing
            // single-target retarget prompt cannot represent multiple groups, so CR 707.10c
            // retargeting remains available only for nonmodal copies for now.
            let needs_target_choice = !src.targets.is_empty() && src.chosen_modes.is_empty();
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
                    ability_index: None,
                    is_triggered: false,
                    is_copy: true,
                    chosen_x: src.chosen_x,
                    face_index: src.face_index,
                    target_damage: src.target_damage.clone(),
                    chosen_modes: src.chosen_modes.clone(),
                    // CR 707.2: the copy has the original's characteristics and choices. `None`
                    // for every spell today, but copying inherits it rather than dropping it.
                    trigger_player: src.trigger_player,
                    flashback: false,
                };
                // CR 707.10c: prompt for new targets on the first copy; push any
                // additional copies immediately with the original targets.
                if needs_target_choice && copy_num == 0 {
                    let sp = compute_spell_targets(engine, controller, &src_effects);
                    let mut candidates: Vec<ObjectId> = sp.valid_permanent_ids.clone();
                    candidates.extend(sp.valid_stack_ids.iter().copied());
                    for p in &engine.state.players {
                        if (sp.can_target_self && p.id == controller)
                            || (sp.can_target_opponent && p.id != controller)
                        {
                            candidates.push(p.id as ObjectId);
                        }
                    }
                    // CR 707.10c: may keep original targets even if now illegal.
                    for &ot in &src.targets {
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
                                min: 1,
                                max: 1,
                                ordered: false,
                                unique_names: false,
                                candidate_server_card_ids: Vec::new(),
                            },
                        )),
                    });
                    events.push(ev_log(prompt.clone()));
                    engine.state.pending_resolution = Some(PendingResolution {
                        item: copy_template,
                        custom_key: "__copy_targets".to_string(),
                        step: 0,
                        scratch: vec![],
                        deciding_player: controller,
                        candidates,
                        min: 1,
                        max: 1,
                        ordered: false,
                        prompt,
                        choice_kind: custom::ChoiceKind::TargetObjects,
                        unique_names: false,
                        copy_source_object_id: src.id,
                        search_destination: SearchDestination::Hand,
                        search_shuffle: false,
                        search_reveal: false,
                        // The parked item is the *copy* being retargeted, not the effect list this
                        // runs inside, so there is no tail here to resume. `CopyTargetSpell`
                        // itself returns `Continue`, and its own list carries on normally.
                        resume_effect_index: None,
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
                                .map(|&o| rv1::TargetRef {
                                    object_id: o,
                                    damage_amount: 0,
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
                }
            }
        }
    }

    Ok(EffectOutcome::Continue)
}
