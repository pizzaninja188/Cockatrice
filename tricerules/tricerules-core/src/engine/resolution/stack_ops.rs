use super::*;

pub(super) fn counter_target_spell(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CounterTargetSpell {
        unless_controller_pays,
        unless_controller_pays_by_cast_cost,
        ..
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let parked_stack = cx.parked_stack();
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let targets = cx.targets;
    let spell_label = cx.spell_label;

    if let Some(&tid) = targets.first() {
        // A resolving program shares one target snapshot across its instructions. An earlier
        // instruction may have removed this spell before a later counter starts; compare the
        // captured generation before looking up its ObjectId so a same-id re-entry cannot satisfy
        // the old target. Modal targets are stored on the chosen mode, not `top.targets`.
        let matching_target_refs = cx
            .target_receipts
            .iter()
            .filter(|target| target.object_id == tid)
            .collect::<Vec<_>>();
        let target_identity_is_current =
            counter_target_refs_are_current(engine, &matching_target_refs);
        let targeted_spell_controller = target_identity_is_current
            .then(|| engine.state.stack.iter().find(|item| item.id == tid))
            .flatten()
            .map(|item| item.controller);
        cx.effect_result.program_value = Some(crate::state::ProgramValue::TargetedSpellController(
            targeted_spell_controller,
        ));
        if !target_identity_is_current {
            if unless_controller_pays.is_some() {
                cx.effect_result.counter_payment_outcome =
                    Some(tricerules_card_model::primitives::CounterPaymentOutcome::NotOffered);
            }
            return Ok(EffectOutcome::Continue);
        }
        let receipt_payment = unless_controller_pays_by_cast_cost.map(|conditional| {
            if cx.top.cast_cost_condition_matches(&conditional.condition) {
                conditional.if_selected
            } else {
                conditional.otherwise
            }
        });
        let payment = unless_controller_pays.as_ref().map(|amount| {
            engine.resolve_amount(
                amount,
                AmountContext::for_stack_item(cx.top, cx.controller)
                    .with_previous_effect_result(cx.previous_effect_result),
            )
        });
        if let Some(generic_mana_cost) = payment.or(receipt_payment) {
            let Some(target) = engine
                .state
                .stack
                .iter()
                .find(|item| item.id == tid)
                .cloned()
            else {
                if unless_controller_pays.is_some() {
                    cx.effect_result.counter_payment_outcome =
                        Some(tricerules_card_model::primitives::CounterPaymentOutcome::NotOffered);
                }
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
                        resolution_prompt_id: 0,
                        accepted_permanent_selections: Vec::new(),
                        variable_mana_contribution: false,
                        candidate_token_identities: Vec::new(),
                        candidate_player_ids: Vec::new(),
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
                        public_reveal: None,
                        candidate_source_zones: Vec::new(),
                        combat_defender_options: Vec::new(),
                        waterbend: false,
                        selection_slots: Vec::new(),
                        replacement_options: Vec::new(),
                        selection_alternatives: Vec::new(),
                    },
                )),
            });
            engine.state.pending_resolution = Some(PendingResolution {
                resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
                resolution_prompt_semantics: None,
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
                    stack: parked_stack,
                    payment: PendingManaPayment {
                        waterbend: false,
                        variable_mana_contribution: false,
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
        counter_stack_object(engine, tid, spell_label, events)?;
    } else if unless_controller_pays.is_some() {
        cx.effect_result.counter_payment_outcome =
            Some(tricerules_card_model::primitives::CounterPaymentOutcome::NotOffered);
    }

    Ok(EffectOutcome::Continue)
}

fn counter_target_refs_are_current(
    engine: &GameEngine,
    target_refs: &[&crate::state::StackTarget],
) -> bool {
    !target_refs.is_empty()
        && target_refs
            .iter()
            .all(|target| stack_target_identity_is_current(engine, target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_counter_does_not_offer_to_same_object_id_with_newer_generation() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            159_008,
            &[0, 1],
            20,
            Some(vec![
                vec!["opt".into(); 20],
                vec!["counterspell".into(); 20],
            ]),
            true,
        )
        .expect("new test engine");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.priority_idx = 0;
        engine.state.players[0].mana_pool.blue = 1;
        engine.state.players[1].mana_pool.blue = 2;
        engine
            .apply_scenario_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                        cast_method: rv1::CastMethod::Normal as i32,
                        source: Some(rv1::CastSource {
                            location: Some(rv1::cast_source::Location::HandIndex(0)),
                            ..Default::default()
                        }),
                        ..Default::default()
                    })),
                },
            )
            .expect("cast the target spell");
        let target_id = engine.state.stack.last().expect("target spell").id;
        engine
            .apply_scenario_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(
                        rv1::PassPriority::default(),
                    )),
                },
            )
            .expect("pass priority to the counter spell's controller");
        let captured_generation = engine
            .state
            .zone_change_generation
            .get(&target_id)
            .copied()
            .unwrap_or(0);
        engine
            .apply_scenario_command(
                1,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                        cast_method: rv1::CastMethod::Normal as i32,
                        source: Some(rv1::CastSource {
                            location: Some(rv1::cast_source::Location::HandIndex(0)),
                            ..Default::default()
                        }),
                        targets: vec![rv1::TargetRef {
                            object_id: target_id,
                            group_index: 0,
                            kind: rv1::TargetRefKind::Stack as i32,
                            expected_zone_change_generation: Some(captured_generation),
                            ..Default::default()
                        }],
                        ..Default::default()
                    })),
                },
            )
            .expect("cast a counter spell targeting the captured object");
        let top = engine.state.stack.last().expect("counter spell").clone();
        assert_eq!(
            top.targets[0].zone_change_generation,
            Some(captured_generation)
        );
        engine
            .state
            .zone_change_generation
            .insert(target_id, captured_generation + 1);
        assert!(engine.state.stack.iter().any(|item| item.id == target_id));

        let prior = EffectResult::default();
        let mut result = EffectResult::default();
        let mut events = Vec::new();
        let targets = [target_id];
        let outcome = {
            let mut cx = EffectCx {
                engine: &mut engine,
                events: &mut events,
                targets: &targets,
                targets_by_role: &[],
                target_receipts: &[],
                target_role_group_indices: &[],
                top: &top,
                controller: top.controller,
                affected_player: top.controller,
                spell_label: "Counterspell fixture",
                previous_effect_result: &prior,
                effect_result: &mut result,
                effect_index: 0,
                resume_previous_result: None,
                program_state: None,
            };
            counter_target_spell(
                &mut cx,
                SpellEffectKind::CounterTargetSpell {
                    spell_filter: Default::default(),
                    unless_controller_pays: Some(tricerules_card_model::Amount::Fixed(2)),
                    unless_controller_pays_by_cast_cost: None,
                },
            )
            .expect("resolve fixed-cost counter against stale captured incarnation")
        };

        assert!(matches!(outcome, EffectOutcome::Continue));
        assert_eq!(
            result.counter_payment_outcome,
            Some(tricerules_card_model::primitives::CounterPaymentOutcome::NotOffered)
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.iter().any(|item| item.id == target_id));
        assert!(!events.iter().any(|event| matches!(
            event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
        )));
    }
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
    let deciding_player = cx
        .top
        .trigger_context
        .affected_player
        .ok_or(EngineError::Illegal(
            "Ward payer missing from trigger context",
        ))?;
    let ward_text = cx.top.ability_text.as_deref().unwrap_or("Ward").to_string();
    if cx
        .engine
        .state
        .player_idx(deciding_player)
        .is_none_or(|index| cx.engine.state.players[index].has_lost)
    {
        counter_stack_object_ref(cx.engine, target, &ward_text, cx.events)?;
        cx.effect_result.counter_payment_outcome =
            Some(tricerules_card_model::primitives::CounterPaymentOutcome::ForcedUnpaid);
        return Ok(EffectOutcome::Continue);
    }
    let prompt = match &cost {
        ResolutionCost::Mana(mana) => format!("Pay {mana} for {ward_text}?"),
        ResolutionCost::DiscardCard { .. } => {
            format!("Discard a matching card to pay for {ward_text}, or decline.")
        }
        ResolutionCost::Waterbend(_)
        | ResolutionCost::None
        | ResolutionCost::Blight { .. }
        | ResolutionCost::ExileGraveyardCard { .. }
        | ResolutionCost::PutHandCardOnLibraryBottom
        | ResolutionCost::SacrificePermanent { .. }
        | ResolutionCost::TapPermanents { .. } => {
            return Err(EngineError::Illegal("unsupported Ward cost"));
        }
    };

    let (presentation, stage, event) = match cost {
        ResolutionCost::Mana(mana_cost) => {
            let payment = PendingManaPayment::from_cost(
                target.object_id,
                mana_cost.clone(),
                cx.engine.state.undoable_mana_abilities.len(),
            );
            let generic_mana_cost = payment.generic_mana_cost;
            let payment_mana_cost = &payment.mana_cost;
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
                resolution_prompt_id: 0,
                accepted_permanent_selections: Vec::new(),
                variable_mana_contribution: false,
                candidate_token_identities: Vec::new(),
                candidate_player_ids: Vec::new(),
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
                        .can_pay_resolution_mana(deciding_player, payment_mana_cost)
                },
                public_reveal: None,
                candidate_source_zones: Vec::new(),
                combat_defender_options: Vec::new(),
                waterbend: false,
                selection_slots: Vec::new(),
                replacement_options: Vec::new(),
                selection_alternatives: Vec::new(),
            };
            (presentation, PendingWardPaymentStage::Mana(payment), event)
        }
        ResolutionCost::DiscardCard { filter } => {
            let cost = ResolutionCost::DiscardCard { filter };
            let candidates = cx
                .engine
                .resolution_cost_candidates(deciding_player, 0, 0, &cost);
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
                resolution_prompt_id: 0,
                accepted_permanent_selections: Vec::new(),
                variable_mana_contribution: false,
                candidate_token_identities: Vec::new(),
                candidate_player_ids: Vec::new(),
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
                public_reveal: None,
                candidate_source_zones: Vec::new(),
                combat_defender_options: Vec::new(),
                waterbend: false,
                selection_slots: Vec::new(),
                replacement_options: Vec::new(),
                selection_alternatives: Vec::new(),
            };
            (
                presentation,
                PendingWardPaymentStage::Discard {
                    candidate_generations,
                },
                event,
            )
        }
        ResolutionCost::Waterbend(_)
        | ResolutionCost::None
        | ResolutionCost::Blight { .. }
        | ResolutionCost::ExileGraveyardCard { .. }
        | ResolutionCost::PutHandCardOnLibraryBottom
        | ResolutionCost::SacrificePermanent { .. }
        | ResolutionCost::TapPermanents { .. } => unreachable!(),
    };

    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(event)),
    });
    cx.events.push(ev_log(format!(
        "P{deciding_player} must decide whether to pay {ward_text}."
    )));
    cx.engine.state.pending_resolution = Some(PendingResolution {
        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
        resolution_prompt_semantics: None,
        deciding_player,
        presentation,
        continuation: ResolutionContinuation::WardPayment {
            stack: cx.parked_stack(),
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
    counter_stack_object(engine, target.object_id, counter_label, events).map(|_| ())
}

/// Shared physical stack removal. Public authored effects retain their distinct spell/ability
/// vocabulary; this executor returns the exact removed item so a successful counter can feed a
/// generation-bound follow-up instruction without serializing private resolution state.
pub(crate) fn counter_stack_object(
    engine: &mut GameEngine,
    target_id: ObjectId,
    counter_label: &str,
    events: &mut Vec<rv1::RuledEvent>,
) -> Result<Option<StackItem>, EngineError> {
    let Some(pos) = engine
        .state
        .stack
        .iter()
        .position(|item| item.id == target_id)
    else {
        return Ok(None);
    };
    let target = engine.state.stack[pos].clone();
    let target_name = engine
        .state
        .stack_presentations
        .get(&target_id)
        .and_then(|presentation| presentation.source_label.clone())
        .unwrap_or_else(|| {
            engine
                .registry
                .get(&target.card_id)
                .map(|definition| definition.name.as_str())
                .unwrap_or("spell")
                .to_owned()
        });
    let cannot_be_countered = target.ability_text.is_none()
        && engine
            .registry
            .get(&target.card_id)
            .and_then(|definition| definition.face(target.face_index))
            .is_some_and(|face| {
                face.static_abilities.iter().any(|ability| {
                    matches!(ability.definition, StaticAbilityDef::SpellCannotBeCountered)
                })
            })
        || controller_static_protects_spell(engine, &target)
        || engine
            .state
            .players
            .iter()
            .flat_map(|player| player.battlefield.iter())
            .any(|&source| {
                engine
                    .active_static_ability_definitions(source)
                    .into_iter()
                    .any(|definition| {
                        matches!(
                            definition,
                            StaticAbilityDef::SpellsAndAbilitiesCannotBeCountered
                        )
                    })
            });
    if cannot_be_countered {
        events.push(ev_log(format!(
            "{counter_label} cannot counter {target_name}"
        )));
        return Ok(None);
    }
    let st = engine.state.stack.remove(pos);
    if let Some(key) = st.trigger_context.memory_jar_cohort {
        engine.state.memory_jar_cohorts.remove(&key);
    }
    engine.refresh_captured_activated_ability(&st);
    engine.state.captured_activated_abilities.remove(&target_id);
    engine
        .state
        .millennium_calendar_state_trigger_stack_keys
        .remove(&target_id);
    if st
        .triggered_ability
        .as_ref()
        .is_some_and(|ability| ability.effect.as_slice() == [SpellEffectKind::ExileObservedObjects])
    {
        if let Some(observed) = st.trigger_context.observed_object {
            engine
                .state
                .observed_object_cohorts
                .remove(&(observed.object_id, observed.zone_change_generation));
        }
    }
    engine.state.captured_spell_copies.remove(&target_id);
    events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::StackObjectCountered(
            rv1::StackObjectCountered {
                object_id: target_id,
            },
        )),
    });
    if st.ability_text.is_none() && !st.is_copy {
        let owner = engine.state.objects.get(&st.id).map(|object| object.owner);
        let destination = if st.cast_method.exiles_on_leave_stack() {
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
                if st.cast_method.exiles_on_leave_stack() {
                    rv1::permanent_moved::Destination::Exile
                } else {
                    rv1::permanent_moved::Destination::Graveyard
                },
            ));
        }
    }
    events.push(ev_log(format!("{counter_label} counters {target_name}")));
    engine.check_millennium_calendar_state_triggers();
    Ok(Some(st))
}

/// Chimil and Rhythm of the Wild protect spells by their live controller and the spell's
/// current stack characteristics. This check belongs at counter resolution; it does not affect
/// target legality, so counterspells can still target those spells and finish their other effects.
fn controller_static_protects_spell(engine: &GameEngine, target: &StackItem) -> bool {
    if target.ability_text.is_some() {
        return false;
    }
    engine
        .state
        .players
        .iter()
        .flat_map(|player| player.battlefield.iter().copied())
        .any(|source_id| {
            if engine.controller_of(source_id) != Some(target.controller) {
                return false;
            }
            engine
                .active_static_ability_definitions(source_id)
                .into_iter()
                .any(|definition| {
                    let StaticAbilityDef::ControllerSpellsCannotBeCountered { spell_filter } =
                        definition
                    else {
                        return false;
                    };
                    super::super::targeting::stack_spell_target_legal(
                        &engine.state,
                        engine.registry,
                        target.id,
                        &spell_filter,
                    )
                })
        })
}

pub(super) fn counter_target_ability(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CounterTargetAbility = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let Some(&target_id) = cx.targets.first() else {
        return Ok(EffectOutcome::Continue);
    };
    let Some(countered) = counter_stack_object(cx.engine, target_id, cx.spell_label, cx.events)?
    else {
        return Ok(EffectOutcome::Continue);
    };
    if let Some(source_id) = countered.source_permanent_id {
        cx.effect_result.produced_objects.push(TriggerObjectRef {
            object_id: source_id,
            zone_change_generation: countered.source_zone_change,
            controller_at_event: countered.controller,
        });
    }
    Ok(EffectOutcome::Continue)
}

pub(super) fn copy_triggering_activated_ability(
    cx: &mut EffectCx<'_>,
) -> Result<EffectOutcome, EngineError> {
    let reference =
        cx.top
            .trigger_context
            .activated_stack_object
            .ok_or(EngineError::ProgramInvariant(
                "copy trigger lacks an activated-ability occurrence",
            ))?;
    // The parent Program can resume after a copy-target prompt and must still prove its input.
    // Its normal stack-exit path, rather than the copy operation, retires this receipt.
    let receipt = cx
        .engine
        .state
        .captured_activated_abilities
        .get(&cx.top.id)
        .cloned()
        .ok_or(EngineError::ProgramInvariant(
            "copy trigger lacks its activated-ability receipt",
        ))?;
    if reference.zone_change_generation.is_some() || reference.object_id != receipt.item.id {
        return Err(EngineError::ProgramInvariant(
            "copy trigger receipt identity mismatch",
        ));
    }
    let live = cx.engine.state.stack.iter().find(|item| {
        item.id == reference.object_id && !item.is_copy && item.activated_ability.is_some()
    });
    let original = live.cloned().unwrap_or(receipt.item);
    let presentation = live
        .and_then(|item| cx.engine.state.stack_presentations.get(&item.id))
        .cloned()
        .unwrap_or(receipt.presentation);
    let copy_id = cx.engine.state.next_object_id;
    cx.engine.state.next_object_id += 1;
    let mut copy = original.clone();
    copy.id = copy_id;
    copy.controller = cx.controller;
    copy.is_copy = true;
    copy.mana_colors_spent_to_cast = Default::default();
    copy.cast_condition_results.clear();
    copy.cast_occurrence = None;
    copy.cast_by = None;
    copy.exiled_cohorts.clear();
    copy.chaos_warp_owner_instructions.clear();
    copy.resolution_branch_choices.clear();
    cx.engine
        .state
        .stack_presentations
        .insert(copy_id, presentation.clone());

    if !copy.targets.is_empty()
        || copy
            .chosen_modes
            .iter()
            .any(|mode| !mode.targets.is_empty())
    {
        let parent = Box::new(cx.parked_stack());
        cx.engine.begin_copy_target_choices(
            vec![ParkedStackResolution::new(copy)],
            original.id,
            Some(parent),
            cx.events,
        )?;
        return Ok(EffectOutcome::Suspended);
    }

    let description = presentation
        .source_label
        .clone()
        .unwrap_or_else(|| "Ability".to_string());
    let annotation = copy
        .ability_text
        .as_ref()
        .map(|text| format!("{text} (copy)"))
        .unwrap_or_else(|| "(copy)".to_string());
    let ability_source = copy.source_permanent_id.and_then(|source_id| {
        cx.engine
            .public_stack_ability_source(source_id, copy.source_zone_change)
    });
    cx.engine.state.stack.push(copy);
    cx.engine.state.passes_since_stack_change = 0;
    cx.events.push(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
            object_id: copy_id,
            description: description.clone(),
            targets: Vec::new(),
            ability_annotation: annotation,
            card_id: String::new(),
            card_display_name: String::new(),
            is_prepare_spell: false,
            is_copy: true,
            is_triggered: false,
            copy_source_object_id: original.id,
            chosen_mode_indices: Vec::new(),
            chosen_mode_labels: Vec::new(),
            chosen_cast_cost_labels: Vec::new(),
            source_token_identity: presentation.source_token_identity,
            primary_presentation: presentation.primary,
            chosen_mode_presentations: presentation.chosen_modes,
            chosen_cast_cost_presentations: presentation.chosen_cast_costs,
            prototyped: false,
            ability_controller_player_id: Some(cx.controller),
            ability_source,
        })),
    });
    cx.events.push(ev_log(format!(
        "{} copies {description} (P{})",
        cx.spell_label, cx.controller
    )));
    cx.engine.fire_triggers(
        &[GameEvent::TargetsChosen {
            controller: cx.controller,
            source: TargetingSourceKind::Ability,
            stack_object: StackObjectRef {
                object_id: copy_id,
                zone_change_generation: None,
            },
            targets: Vec::new(),
        }],
        cx.events,
    );
    Ok(EffectOutcome::Continue)
}

pub(super) fn copy_target_spell(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let (count, original) = match effect {
        SpellEffectKind::CopyTargetSpell { count, .. } => (
            count,
            cx.targets
                .first()
                .and_then(|id| cx.engine.state.stack.iter().find(|spell| spell.id == *id))
                .cloned(),
        ),
        SpellEffectKind::CopyCapturedSpell { count } => (
            count,
            cx.engine.state.captured_spell_copies.remove(&cx.top.id),
        ),
        _ => return Err(EngineError::Illegal("resolution dispatch mismatch")),
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    // CR 707.10: create `count` copies of the target spell on the stack, each
    // controlled by this spell's controller. The copy is not cast (no mana/cast
    // triggers) and ceases to exist after resolving (handled by `is_copy`). It uses
    // the original's chosen X and face. CR 707.10c: the copy's controller may choose
    // new targets for each copy; if the copied spell has targets, we prompt before
    // placing the copy on the stack. Targeted copies are queued so every copy
    // gets its own choice before priority returns.
    if let Some(src) = original {
        let copied_name = engine
            .registry
            .get(&src.card_id)
            .and_then(|d| d.face(src.face_index))
            .map(|f| f.name.to_string())
            .unwrap_or_else(|| src.card_id.clone());
        let target_count = if src.chosen_modes.is_empty() {
            src.targets.len()
        } else {
            src.chosen_modes.iter().map(|mode| mode.targets.len()).sum()
        };
        let needs_target_choice = target_count > 0;
        let copied_face = engine
            .registry
            .get(&src.card_id)
            .and_then(|definition| definition.face(src.face_index));
        let copied_modal = copied_face.and_then(|face| face.modal_spell.as_ref());
        let (chosen_mode_indices, chosen_mode_labels) = if src.chosen_modes.is_empty() {
            (Vec::new(), Vec::new())
        } else {
            let modal = copied_modal.ok_or(EngineError::Illegal(
                "copied modal spell has no mode definition",
            ))?;
            let indices = src
                .chosen_modes
                .iter()
                .map(|chosen| {
                    modal
                        .mode_index(&chosen.mode_id)
                        .map(|index| index as u32)
                        .ok_or(EngineError::Illegal("copied modal mode no longer exists"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let labels = src
                .chosen_modes
                .iter()
                .map(|chosen| {
                    modal
                        .mode_by_id(&chosen.mode_id)
                        .map(|mode| mode_fallback(&copied_name, &mode.mode_id))
                        .ok_or(EngineError::Illegal("copied modal mode no longer exists"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            (indices, labels)
        };
        let mut target_choice_copies = Vec::new();
        for _ in 0..count {
            let copy_id = engine.state.next_object_id;
            engine.state.next_object_id += 1;
            let copy_presentation = engine
                .state
                .stack_presentations
                .get(&src.id)
                .cloned()
                .unwrap_or_default();
            engine
                .state
                .stack_presentations
                .insert(copy_id, copy_presentation.clone());
            let copy_template = StackItem {
                mana_colors_spent_to_cast: Default::default(),
                id: copy_id,
                controller,
                card_id: src.card_id.clone(),
                prototyped: src.prototyped,
                targets: src.targets.clone(),
                ability_text: None,
                source_permanent_id: None,
                source_owner: None,
                source_zone_change: 0,
                trigger_use_key: None,
                source_face_change: 0,
                ability_index: None,
                activated_ability: None,
                activated_ability_occurrence: None,
                triggered_ability: None,
                is_triggered: false,
                is_copy: true,
                chosen_x: src.chosen_x,
                face_index: src.face_index,
                chosen_modes: src.chosen_modes.clone(),
                cast_condition_results: Vec::new(),
                cast_occurrence: None,
                cast_by: None,
                cast_cost_receipts: src.cast_cost_receipts.clone(),
                payment_result: src.payment_result.clone(),
                exiled_cohorts: Default::default(),
                chaos_warp_owner_instructions: Default::default(),
                resolution_branch_choices: Default::default(),
                blight_receipts: src.blight_receipts.clone(),
                // CR 707.2: the copy has the original's characteristics and choices. `None`
                // for every spell today, but copying inherits it rather than dropping it.
                trigger_context: src.trigger_context,
                // CR 707.10 copies the Warp choice; Flashback/Harmonize stack-exit
                // replacements still belong only to the spell actually cast that way.
                cast_method: if matches!(
                    src.cast_method,
                    SpellCastMethod::Warp | SpellCastMethod::Sneak
                ) {
                    src.cast_method
                } else {
                    SpellCastMethod::Normal
                },
                returned_attacker_assignment: src.returned_attacker_assignment,
            };
            if needs_target_choice {
                target_choice_copies.push(ParkedStackResolution::new(copy_template));
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
                                expected_zone_change_generation: target.zone_change_generation,
                            })
                            .collect(),
                        ability_annotation: "(copy)".to_string(),
                        card_id: src.card_id.clone(),
                        card_display_name: engine
                            .registry
                            .get(&src.card_id)
                            .and_then(|definition| definition.face_display_name(src.face_index))
                            .unwrap_or(&copied_name)
                            .to_string(),
                        is_prepare_spell: engine.registry.get(&src.card_id).is_some_and(|d| {
                            d.layout == Layout::Preparation && src.face_index == 1
                        }),
                        is_copy: true,
                        is_triggered: false,
                        copy_source_object_id: src.id,
                        chosen_mode_indices: chosen_mode_indices.clone(),
                        chosen_mode_labels: chosen_mode_labels.clone(),
                        chosen_cast_cost_labels: src
                            .cast_cost_receipts
                            .iter()
                            .map(|receipt| receipt.label.clone())
                            .collect(),
                        source_token_identity: None,
                        primary_presentation: copy_presentation.primary,
                        chosen_mode_presentations: copy_presentation.chosen_modes,
                        chosen_cast_cost_presentations: copy_presentation.chosen_cast_costs,
                        prototyped: src.prototyped,
                        ability_controller_player_id: None,
                        ability_source: None,
                    })),
                });
                events.push(ev_log(format!(
                    "{spell_label} copies {copied_name} (P{controller})"
                )));
                engine.fire_triggers(
                    &[GameEvent::TargetsChosen {
                        controller,
                        source: TargetingSourceKind::SpellCopy,
                        stack_object: StackObjectRef {
                            object_id: copy_id,
                            zone_change_generation: None,
                        },
                        targets: src.targets.clone(),
                    }],
                    events,
                );
            }
        }
        if !target_choice_copies.is_empty() {
            engine.begin_copy_target_choices(target_choice_copies, src.id, None, events)?;
        }
    }

    Ok(EffectOutcome::Continue)
}

#[cfg(test)]
mod cast_color_tests {
    use super::*;

    /// Internal executor coverage: no registered card currently copies permanent spells.
    #[test]
    fn pentad_prism_uncast_copy_factory_clears_actual_paid_colors_before_token_entry() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            202_610_240,
            &[0, 1],
            20,
            Some(vec![
                vec!["pentad_prism".into(); 20],
                vec!["island".into(); 20],
            ]),
            true,
        )
        .unwrap();
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.priority_idx = 0;
        engine.state.players[0].mana_pool.white = 1;
        engine.state.players[0].mana_pool.blue = 1;
        engine
            .apply_scenario_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                        cast_method: rv1::CastMethod::Normal as i32,
                        source: Some(rv1::CastSource {
                            location: Some(rv1::cast_source::Location::HandIndex(0)),
                            ..Default::default()
                        }),
                        ..Default::default()
                    })),
                },
            )
            .unwrap();
        let original = engine.state.stack.last().unwrap().clone();
        assert_eq!(original.mana_colors_spent_to_cast.count(), 2);
        assert!(original.cast_occurrence.is_some());
        let mut events = Vec::new();
        let mut result = EffectResult::default();
        let prior = EffectResult::default();
        let targets = [original.id];
        let mut cx = EffectCx {
            engine: &mut engine,
            events: &mut events,
            targets: &targets,
            targets_by_role: &[],
            target_receipts: &[],
            target_role_group_indices: &[],
            top: &original,
            controller: 0,
            affected_player: 0,
            spell_label: "internal copy fixture",
            previous_effect_result: &prior,
            effect_result: &mut result,
            effect_index: 0,
            resume_previous_result: None,
            program_state: None,
        };
        copy_target_spell(
            &mut cx,
            SpellEffectKind::CopyTargetSpell {
                count: 1,
                spell_filter: Default::default(),
            },
        )
        .unwrap();
        let copy = engine.state.stack.last().unwrap();
        let copy_id = copy.id;
        assert_ne!(copy_id, original.id);
        assert!(copy.is_copy);
        assert!(copy.cast_occurrence.is_none());
        assert_eq!(copy.mana_colors_spent_to_cast.count(), 0);
        assert!(!engine.state.objects.contains_key(&copy_id));
        assert_eq!(engine.state.stack[0].mana_colors_spent_to_cast.count(), 2);
        for expected in [copy_id, original.id] {
            for _ in 0..2 {
                let actor = engine.state.priority_player_id();
                engine
                    .apply_scenario_command(
                        actor,
                        &rv1::RuledCommand {
                            cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                        },
                    )
                    .unwrap();
            }
            assert_eq!(engine.state.objects[&expected].zone, Zone::Battlefield);
            assert_eq!(
                engine.state.objects[&expected].counter_count(CounterKind::Charge),
                if expected == copy_id { 0 } else { 2 }
            );
            if expected == copy_id {
                assert!(engine.state.objects[&expected].is_token());
                assert_eq!(engine.state.objects[&original.id].zone, Zone::Stack);
            }
        }
        assert!(engine.state.stack.is_empty());
    }
}
