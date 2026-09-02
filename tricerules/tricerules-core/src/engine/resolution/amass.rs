use super::*;
use crate::engine::events::finish_with_events;
use tricerules_cards::primitives::{PermanentTypeFilter, TypeLineAddition};

pub(super) fn amass(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::Amass { subtype, count } = effect else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let count = cx.engine.resolve_amount(
        &count,
        AmountContext::for_stack_item(cx.top, cx.controller)
            .with_previous_effect_result(cx.previous_effect_result),
    );
    let mut candidates = cx.engine.amass_candidates(cx.controller);
    if candidates.is_empty() {
        let item = cx.top.clone();
        let (entries, logs) = cx.engine.prepare_token_entries(
            TokenCreationRequest {
                token_id: subtype.token_id(),
                values: None,
                count: 1,
                recipients: vec![cx.controller],
                spell_label: cx.spell_label,
                item: &item,
            },
            false,
        )?;
        if cx.engine.begin_token_entry_batch(
            item,
            entries,
            logs,
            TokenEntryBatchOptions {
                amass: Some(PendingAmass { subtype, count }),
                ..Default::default()
            },
            cx.events,
        )? {
            return Ok(EffectOutcome::Suspended);
        }
        candidates = cx.engine.amass_candidates(cx.controller);
    }

    match candidates.as_slice() {
        [] => Ok(EffectOutcome::Continue),
        [chosen] => {
            *cx.effect_result = cx.engine.apply_amass(
                *chosen,
                cx.controller,
                PendingAmass { subtype, count },
                cx.top.id,
                cx.events,
            )?;
            Ok(EffectOutcome::Continue)
        }
        _ => {
            cx.engine.park_amass_choice(
                ParkedStackResolution::new(cx.top.clone()),
                cx.controller,
                PendingAmass { subtype, count },
                candidates,
                cx.spell_label,
                cx.events,
            );
            Ok(EffectOutcome::Suspended)
        }
    }
}

impl GameEngine {
    fn is_amass_candidate(&self, oid: ObjectId, controller: PlayerId) -> bool {
        self.state.objects.get(&oid).is_some_and(|object| {
            object.zone == Zone::Battlefield
                && self.characteristics(oid).is_some_and(|characteristics| {
                    characteristics.controller == controller
                        && characteristics.is_creature()
                        && characteristics.has_type("Army")
                })
        })
    }

    fn amass_candidates(&self, controller: PlayerId) -> Vec<ObjectId> {
        let mut candidates = self
            .state
            .objects
            .keys()
            .copied()
            .filter(|oid| self.is_amass_candidate(*oid, controller))
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        candidates
    }

    fn apply_amass(
        &mut self,
        chosen: ObjectId,
        controller: PlayerId,
        amass: PendingAmass,
        source_id: ObjectId,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<EffectResult, EngineError> {
        if !self.is_amass_candidate(chosen, controller) {
            return Err(EngineError::Illegal("stale Amass Army choice"));
        }
        self.place_counters(chosen, CounterKind::PlusOnePlusOne, amass.count);
        if !self
            .characteristics(chosen)
            .is_some_and(|characteristics| characteristics.has_type(amass.subtype.as_str()))
        {
            self.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: Some(source_id),
                affected: AffectedScope::Single(chosen),
                kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
                    card_types: Vec::<PermanentTypeFilter>::new(),
                    creature_types: vec![amass.subtype.as_str().to_string()],
                }),
                condition: None,
                duration: EffectDuration::Indefinite,
                timestamp: self.state.command_index,
            });
        }
        let generation = self
            .state
            .zone_change_generation
            .get(&chosen)
            .copied()
            .unwrap_or(0);
        events.push(ev_log(format!(
            "P{controller} amasses {} {} on {}.",
            amass.subtype.plural(),
            amass.count,
            object_display_name(&self.state, self.registry, chosen)
        )));
        Ok(EffectResult {
            selected_objects: vec![TriggerObjectRef {
                object_id: chosen,
                zone_change_generation: generation,
                controller_at_event: controller,
            }],
            ..Default::default()
        })
    }

    fn park_amass_choice(
        &mut self,
        stack: ParkedStackResolution,
        controller: PlayerId,
        amass: PendingAmass,
        candidates: Vec<ObjectId>,
        spell_label: &str,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let prompt = format!("Choose an Army you control to amass ({}).", spell_label);
        let candidate_card_ids = candidates
            .iter()
            .map(|oid| {
                self.state
                    .objects
                    .get(oid)
                    .map(|object| object.card_id.clone())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        let candidate_names = candidates
            .iter()
            .map(|oid| object_display_name(&self.state, self.registry, *oid))
            .collect::<Vec<_>>();
        let candidate_generations = candidates
            .iter()
            .map(|oid| {
                (
                    *oid,
                    self.state
                        .zone_change_generation
                        .get(oid)
                        .copied()
                        .unwrap_or(0),
                )
            })
            .collect();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: controller,
                    source_object_id: stack.item.id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::PermanentObjects as i32,
                    candidate_object_ids: candidates.clone(),
                    candidate_card_ids,
                    candidate_names,
                    min: 1,
                    max: 1,
                    ..Default::default()
                },
            )),
        });
        events.push(ev_log(prompt.clone()));
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player: controller,
            presentation: PendingResolutionPresentation {
                source_object_id: stack.item.id,
                candidates,
                min: 1,
                max: 1,
                ordered: false,
                prompt,
                choice_kind: rv1::ChoiceKind::PermanentObjects,
                unique_names: false,
            },
            continuation: ResolutionContinuation::AmassChoice {
                stack,
                subtype: amass.subtype,
                count: amass.count,
                candidate_generations,
            },
        });
    }

    pub(in crate::engine) fn finish_amass_choice(
        &mut self,
        pending: PendingResolution,
        chosen: ObjectId,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::AmassChoice {
            stack,
            subtype,
            count,
            candidate_generations,
        } = &pending.continuation
        else {
            return Err(EngineError::Illegal("not an Amass choice"));
        };
        let generation = self
            .state
            .zone_change_generation
            .get(&chosen)
            .copied()
            .unwrap_or(0);
        if !candidate_generations.contains(&(chosen, generation))
            || !self.is_amass_candidate(chosen, pending.deciding_player)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("stale Amass Army choice"));
        }
        let stack = stack.clone();
        let subtype = *subtype;
        let count = *count;
        let mut events = Vec::new();
        let result = self.apply_amass(
            chosen,
            pending.deciding_player,
            PendingAmass { subtype, count },
            stack.item.id,
            &mut events,
        )?;
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            result,
            events,
        )
    }

    pub(in crate::engine) fn finish_amass_after_token_entry(
        &mut self,
        stack: ParkedStackResolution,
        amass: PendingAmass,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let controller = stack.item.controller;
        let candidates = self.amass_candidates(controller);
        match candidates.as_slice() {
            [] => self.complete_parked_resolution_with_previous(
                stack.item,
                stack.resume_effect_index,
                EffectResult::default(),
                events,
            ),
            [chosen] => {
                let result =
                    self.apply_amass(*chosen, controller, amass, stack.item.id, &mut events)?;
                self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    result,
                    events,
                )
            }
            _ => {
                let label = object_display_name(&self.state, self.registry, stack.item.id);
                self.park_amass_choice(stack, controller, amass, candidates, &label, &mut events);
                Ok(finish_with_events(self, events))
            }
        }
    }
}
