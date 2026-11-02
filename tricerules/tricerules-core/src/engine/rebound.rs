//! Exact-cast Rebound grants and delayed upkeep consumption.
use super::*;
use tricerules_card_model::{AbilityPresentation, SpellCastFilter};

impl GameEngine {
    /// Consume the Rebound opportunities belonging to this actual upkeep and stage each through
    /// the ordinary delayed-trigger path. The exact Exile receipt remains the watched object;
    /// moving the card before this upkeep does not erase the triggered ability.
    pub(super) fn collect_scheduled_rebound_triggers(
        &mut self,
        active_player: PlayerId,
    ) -> Vec<(Option<TriggerObjectRef>, DelayedTriggerPayload)> {
        let scheduled_rebounds = std::mem::take(&mut self.state.scheduled_rebounds);
        let mut retained = Vec::with_capacity(scheduled_rebounds.len());
        let mut delayed = Vec::new();

        for scheduled in scheduled_rebounds {
            if scheduled.controller != active_player {
                retained.push(scheduled);
                continue;
            }

            let ability = TriggeredAbilityDef {
                ability_id: tricerules_card_model::AbilityId::new("cast_rebound")
                    .expect("static ability id"),
                presentation: AbilityPresentation::Fallback,
                trigger: TriggerCondition::AtBeginningOfUpkeep {
                    player: CastTriggerPlayer::Controller,
                },
                effect: vec![SpellEffectKind::CastReboundedCard],
                program: None,
                modal: None,
                targeting: None,
                may: false,
                intervening_if: None,
                triggers_only_once: false,
                max_triggers_per_turn: None,
            };
            let fallback_text = format!(
                "At the beginning of your upkeep, you may cast {} from exile without paying its mana cost.",
                scheduled.card_name
            );
            let presentation = super::presentation::stack_child_presentation_ref(
                self.registry,
                &scheduled.card_id,
                scheduled.source_face_index,
                super::presentation::StackPresentationSource::PhysicalSpell,
                super::presentation::PresentationPath::Ability(&ability.ability_id),
                &ability.presentation,
                fallback_text,
            );

            delayed.push((
                Some(scheduled.exiled),
                DelayedTriggerPayload {
                    source: scheduled.source,
                    controller: scheduled.controller,
                    affected_player: Some(active_player),
                    memory_jar_cohort: None,
                    card_id: scheduled.card_id,
                    card_name: scheduled.card_name,
                    source_face_index: scheduled.source_face_index,
                    presentation,
                    program_definition: None,
                    ability,
                },
            ));
        }

        self.state.scheduled_rebounds = retained;
        delayed
    }

    pub(super) fn register_next_hand_spell_rebound(
        &mut self,
        top: &StackItem,
        controller: PlayerId,
        label: &str,
    ) {
        let source = TriggerObjectRef {
            object_id: top.source_permanent_id.unwrap_or(top.id),
            zone_change_generation: top
                .source_permanent_id
                .map(|_| top.source_zone_change)
                .unwrap_or_else(|| {
                    self.state
                        .zone_change_generation
                        .get(&top.id)
                        .copied()
                        .unwrap_or(0)
                }),
            controller_at_event: controller,
        };
        let ability = TriggeredAbilityDef {
            ability_id: tricerules_card_model::AbilityId::new("grant_captured_spell_rebound")
                .expect("static ability id"),
            presentation: AbilityPresentation::Fallback,
            trigger: TriggerCondition::WheneverPlayerCastsSpell {
                caster: CastTriggerPlayer::Controller,
                filter: SpellCastFilter {
                    card_type: Some(CardTypeFilter::InstantOrSorcery),
                    ..Default::default()
                },
                ordinal: None,
                ordinal_scope: Default::default(),
            },
            effect: vec![SpellEffectKind::GrantCapturedSpellRebound],
            program: None,
            modal: None,
            targeting: None,
            may: false,
            intervening_if: None,
            triggers_only_once: false,
            max_triggers_per_turn: None,
        };
        let presentation = super::presentation::stack_child_presentation_ref(
            self.registry,
            &top.card_id,
            top.face_index,
            super::presentation::StackPresentationSource::for_stack(
                self.state
                    .stack_presentations
                    .get(&top.id)
                    .and_then(|presentation| presentation.primary.as_ref()),
                top.ability_text.is_none(),
            ),
            super::presentation::PresentationPath::Ability(&ability.ability_id),
            &ability.presentation,
            format!("{label} — grant Rebound to the captured spell"),
        );
        self.state.active_event_observers.push(ActiveEventObserver {
            watched: Some(source),
            matcher: EventObserverMatcher::NextHandInstantOrSorceryThisTurn { controller },
            payload: EventObserverPayload::StageDelayedTrigger(Box::new(DelayedTriggerPayload {
                source,
                controller,
                affected_player: None,
                memory_jar_cohort: None,
                card_id: top.card_id.clone(),
                card_name: label.into(),
                source_face_index: top.face_index,
                presentation,
                program_definition: None,
                ability,
            })),
        });
    }

    pub(super) fn grant_captured_spell_rebound(
        &mut self,
        trigger: &StackItem,
    ) -> Result<(), EngineError> {
        let occurrence =
            trigger
                .trigger_context
                .triggering_spell
                .ok_or(EngineError::ProgramInvariant(
                    "GrantCapturedSpellRebound requires its captured spell occurrence",
                ))?;
        let occurrence_is_live = self.state.stack.iter().any(|spell| {
            spell.id == occurrence.object_id
                && spell.cast_occurrence == Some(occurrence)
                && match occurrence.zone_change_generation {
                    Some(generation) => {
                        self.state
                            .objects
                            .get(&occurrence.object_id)
                            .is_some_and(|object| object.zone == Zone::Stack)
                            && self
                                .state
                                .zone_change_generation
                                .get(&occurrence.object_id)
                                .copied()
                                == Some(generation)
                    }
                    None => true,
                }
        });
        if occurrence_is_live {
            self.state
                .rebound_spell_grants
                .insert((occurrence.object_id, occurrence.zone_change_generation));
        }
        Ok(())
    }

    /// Drop only the grant attached to this exact committed cast, preserving later incarnations.
    pub(super) fn clear_rebound_grant_for_stack_item(&mut self, item: &StackItem) {
        if let Some(occurrence) = item.cast_occurrence {
            self.state
                .rebound_spell_grants
                .remove(&(occurrence.object_id, occurrence.zone_change_generation));
            self.state
                .hand_cast_holders
                .remove(&(occurrence.object_id, occurrence.zone_change_generation));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ScenarioCommandExt;
    use tricerules_proto::ruled::v1 as rv1;

    fn narset_phase_b_actual_opt_casts(count: usize) -> (GameEngine, Vec<StackItem>) {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            6_110_201,
            &[0, 1],
            20,
            Some(vec![
                vec!["opt".to_owned(); 20],
                vec!["island".to_owned(); 20],
            ]),
            true,
        )
        .expect("prepared Rebound unit fixture engine");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.priority_idx = 0;
        engine.state.players[0].mana_pool.blue = count as u32;

        for _ in 0..count {
            // Match the existing in-engine Opt hand-cast recipe. Priority is fixture state here;
            // this helper does not claim a normal legal session or a completed Rebound lifecycle.
            engine.state.priority_idx = 0;
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
                .expect("commit an actual Opt from hand");
        }

        let items = engine.state.stack.clone();
        assert_eq!(
            items.len(),
            count,
            "each prepared cast remains on the stack"
        );
        for item in &items {
            assert_eq!(item.card_id, "opt");
            assert!(!item.is_copy, "the test uses an actual cast object");
            let occurrence = item
                .cast_occurrence
                .expect("actual Opt retains its committed cast occurrence");
            assert_eq!(occurrence.object_id, item.id);
            assert!(occurrence.zone_change_generation.is_some());
            assert_eq!(engine.state.objects[&item.id].zone, Zone::Stack);
            assert_eq!(
                engine.state.zone_change_generation[&item.id],
                occurrence
                    .zone_change_generation
                    .expect("physical cast generation")
            );
        }
        (engine, items)
    }

    #[test]
    fn narset_phase_b_prepared_stale_grant_is_noop_and_current_cast_is_positive_control() {
        let (mut engine, items) = narset_phase_b_actual_opt_casts(1);
        let spell = items[0].clone();
        let current = spell
            .cast_occurrence
            .expect("actual Opt has its exact cast occurrence");
        let current_generation = current
            .zone_change_generation
            .expect("physical cast occurrences carry their stack generation");
        let stale = crate::state::StackObjectRef {
            object_id: current.object_id,
            zone_change_generation: Some(
                current_generation
                    .checked_sub(1)
                    .expect("hand-to-stack movement gives Opt a prior generation"),
            ),
        };
        let current_key = (current.object_id, current.zone_change_generation);
        let stale_key = (stale.object_id, stale.zone_change_generation);

        let mut trigger = spell.clone();
        trigger.trigger_context.triggering_spell = Some(stale);
        engine
            .grant_captured_spell_rebound(&trigger)
            .expect("a stale captured occurrence is a no-op");
        assert!(!engine.state.rebound_spell_grants.contains(&stale_key));
        assert!(!engine.state.rebound_spell_grants.contains(&current_key));

        trigger.trigger_context.triggering_spell = Some(current);
        engine
            .grant_captured_spell_rebound(&trigger)
            .expect("the exact live cast accepts the captured grant");
        assert!(engine.state.rebound_spell_grants.contains(&current_key));
        assert!(!engine.state.rebound_spell_grants.contains(&stale_key));
    }

    #[test]
    fn narset_phase_b_prepared_cleanup_removes_only_the_exact_cast_key() {
        let (mut engine, items) = narset_phase_b_actual_opt_casts(1);
        let current_item = items[0].clone();
        let current = current_item
            .cast_occurrence
            .expect("actual Opt has its exact cast occurrence");
        let current_generation = current
            .zone_change_generation
            .expect("physical cast occurrences carry their stack generation");
        let old = crate::state::StackObjectRef {
            object_id: current.object_id,
            zone_change_generation: Some(
                current_generation
                    .checked_sub(1)
                    .expect("hand-to-stack movement gives Opt a prior generation"),
            ),
        };
        let mut old_item = current_item.clone();
        old_item.cast_occurrence = Some(old);

        let old_key = (old.object_id, old.zone_change_generation);
        let current_key = (current.object_id, current.zone_change_generation);
        engine.state.rebound_spell_grants.insert(old_key);
        engine.state.rebound_spell_grants.insert(current_key);
        engine.clear_rebound_grant_for_stack_item(&old_item);
        assert!(!engine.state.rebound_spell_grants.contains(&old_key));
        assert!(engine.state.rebound_spell_grants.contains(&current_key));

        let mut no_generation_item = current_item;
        no_generation_item.cast_occurrence = Some(crate::state::StackObjectRef {
            object_id: current.object_id,
            zone_change_generation: None,
        });
        let no_generation_key = (current.object_id, None);
        let zero_generation_key = (current.object_id, Some(0));
        assert_ne!(no_generation_key, zero_generation_key);
        engine.state.rebound_spell_grants.clear();
        engine.state.rebound_spell_grants.insert(no_generation_key);
        engine
            .state
            .rebound_spell_grants
            .insert(zero_generation_key);
        engine.clear_rebound_grant_for_stack_item(&no_generation_item);
        assert!(!engine
            .state
            .rebound_spell_grants
            .contains(&no_generation_key));
        assert!(engine
            .state
            .rebound_spell_grants
            .contains(&zero_generation_key));
    }

    #[test]
    fn narset_phase_b_prepared_final_exit_clears_departed_grant_and_keeps_another_live_cast() {
        let (mut engine, items) = narset_phase_b_actual_opt_casts(2);
        let departing = items[0].clone();
        let retained = items[1].clone();
        let departing_occurrence = departing
            .cast_occurrence
            .expect("departing Opt has its exact cast occurrence");
        let retained_occurrence = retained
            .cast_occurrence
            .expect("another actual Opt remains a live cast");
        let departing_key = (
            departing_occurrence.object_id,
            departing_occurrence.zone_change_generation,
        );
        let retained_key = (
            retained_occurrence.object_id,
            retained_occurrence.zone_change_generation,
        );
        engine.state.rebound_spell_grants.insert(departing_key);
        engine.state.rebound_spell_grants.insert(retained_key);

        // Prepared state: the physical spell is already in Exile and its stack item has been
        // retired. This does not claim a legal exile action or a complete game session.
        crate::engine::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            departing.id,
            Zone::Exile,
            None,
        )
        .expect("prepare the physical Opt exile before its final stack exit");
        engine.state.stack.retain(|item| item.id != departing.id);
        assert_eq!(engine.state.objects[&departing.id].zone, Zone::Exile);
        assert!(
            !engine
                .state
                .stack
                .iter()
                .any(|item| item.id == departing.id),
            "the prepared departure has already retired the stack item"
        );
        assert!(engine.state.stack.iter().any(|item| item.id == retained.id));
        assert!(engine.state.rebound_spell_grants.contains(&retained_key));

        let mut events = Vec::new();
        engine
            .finish_deferred_stack_exit(
                &departing,
                crate::engine::resolution::DeferredStackExit::Resolved,
                &mut events,
            )
            .expect("final-exit cleanup handles an already-moved physical spell");

        assert!(!engine.state.rebound_spell_grants.contains(&departing_key));
        assert!(engine.state.rebound_spell_grants.contains(&retained_key));
        assert_eq!(engine.state.rebound_spell_grants.len(), 1);
        assert!(!engine
            .state
            .stack
            .iter()
            .any(|item| item.id == departing.id));
        assert!(engine.state.stack.iter().any(|item| item.id == retained.id));
        assert_eq!(engine.state.objects[&departing.id].zone, Zone::Exile);
        assert_eq!(engine.state.objects[&retained.id].zone, Zone::Stack);
        assert!(
            events.is_empty(),
            "the absent stack item takes the early exit"
        );
    }

    #[test]
    fn narset_phase_c2_prepared_copied_rebound_trigger_uses_current_controller() {
        let (mut engine, items) = narset_phase_b_actual_opt_casts(1);
        let spell = items[0].clone();
        let original_controller = spell.controller;
        let copied_controller = engine
            .state
            .players
            .iter()
            .find(|player| player.id != original_controller)
            .expect("the prepared game has an opponent")
            .id;

        let mut grant_trigger = spell.clone();
        grant_trigger.trigger_context.triggering_spell = spell.cast_occurrence;
        engine
            .grant_captured_spell_rebound(&grant_trigger)
            .expect("the exact actual Opt cast can receive its prepared Rebound grant");

        // PREPARED COMPLETED EXIT: use the test-only terminal shim to record the successful
        // Rebound departure. This does not execute Scry/Draw or establish normal session reachability.
        let mut events = Vec::new();
        engine
            .finish_deferred_stack_exit(
                &spell,
                crate::engine::resolution::DeferredStackExit::Resolved,
                &mut events,
            )
            .expect("prepare a resolved Rebound exit through the existing terminal shim");

        assert_eq!(engine.state.scheduled_rebounds.len(), 1);
        let (source, exiled, exiled_owner) = {
            let scheduled = &engine.state.scheduled_rebounds[0];
            let object = engine
                .state
                .objects
                .get(&scheduled.exiled.object_id)
                .expect("prepared Rebound retains its physical Opt");
            assert_eq!(scheduled.controller, original_controller);
            assert_eq!(object.zone, Zone::Exile);
            (scheduled.source, scheduled.exiled, object.owner)
        };
        let stack_generation = spell
            .cast_occurrence
            .expect("actual Opt retains its exact stack occurrence")
            .zone_change_generation
            .expect("physical Opt has its stack generation");
        assert_eq!(source.object_id, spell.id);
        assert_eq!(source.zone_change_generation, stack_generation);
        assert_eq!(source.controller_at_event, original_controller);
        assert_eq!(exiled.object_id, spell.id);
        assert_ne!(exiled.zone_change_generation, stack_generation);
        assert_eq!(exiled.controller_at_event, original_controller);

        let turn_instance = engine.state.turn_instance;
        engine.fire_triggers(
            &[GameEvent::PhaseBegan {
                phase: rv1::PhaseId::Upkeep,
                active_player: original_controller,
            }],
            &mut events,
        );
        engine.flush_staged_triggers(&mut events);
        assert_eq!(engine.state.turn_instance, turn_instance);
        assert!(engine.state.scheduled_rebounds.is_empty());
        assert_eq!(
            engine.state.stack.len(),
            1,
            "one ordinary delayed trigger is staged"
        );

        let delayed_trigger = engine.state.stack[0].clone();
        assert!(delayed_trigger.is_triggered);
        assert!(!delayed_trigger.is_copy);
        assert_eq!(delayed_trigger.controller, original_controller);
        assert_eq!(delayed_trigger.source_permanent_id, Some(source.object_id));
        assert_eq!(
            delayed_trigger.source_zone_change,
            source.zone_change_generation
        );
        assert_eq!(delayed_trigger.source_owner, Some(exiled_owner));
        assert_eq!(
            delayed_trigger.trigger_context.observed_object,
            Some(exiled)
        );

        let copy_id = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        let copied_presentation = engine
            .state
            .stack_presentations
            .get(&delayed_trigger.id)
            .cloned()
            .expect("the ordinary delayed trigger has its stack presentation");
        let mut copied_trigger = delayed_trigger.clone();
        copied_trigger.id = copy_id;
        copied_trigger.controller = copied_controller;
        copied_trigger.is_copy = true;
        engine
            .state
            .stack_presentations
            .insert(copy_id, copied_presentation);
        engine.state.stack.push(copied_trigger);

        let progress = engine
            .resolve_top_of_stack(&mut events)
            .expect("the copied delayed trigger uses its current controller for the offer");
        assert_eq!(
            progress,
            crate::engine::resolution::ResolutionProgress::Parked
        );
        assert_eq!(engine.state.stack.len(), 1);
        assert_eq!(engine.state.stack[0].id, delayed_trigger.id);

        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the copied trigger parks its exact Rebound cast offer");
        assert_eq!(pending.deciding_player, copied_controller);
        let crate::state::ResolutionContinuation::SpecialCast {
            stack,
            exiled: offered_exile,
            method,
            cost,
            permission_scope,
            ..
        } = &pending.continuation
        else {
            panic!("the copied Rebound trigger parks a special-cast offer");
        };
        assert_eq!(stack.item.controller, copied_controller);
        assert_eq!(stack.item.source_permanent_id, Some(source.object_id));
        assert_eq!(stack.item.source_zone_change, source.zone_change_generation);
        assert_eq!(stack.item.source_owner, Some(exiled_owner));
        assert_eq!(stack.item.trigger_context.observed_object, Some(exiled));
        assert_eq!(*offered_exile, exiled);
        assert_eq!(offered_exile.controller_at_event, original_controller);
        assert_eq!(*method, SpellCastMethod::Permission);
        assert_eq!(*permission_scope, Some(ExilePlayPermissionScope::CastCard));
        assert_eq!(cost, &ManaCost::default());

        let permission = engine
            .special_cast_permission(copied_controller)
            .expect("only the copied trigger's current controller receives the offer");
        assert_eq!(permission.player_id, copied_controller);
        assert_eq!(permission.object_id, exiled.object_id);
        assert_eq!(
            permission.zone_change_generation,
            exiled.zone_change_generation
        );
        assert_eq!(permission.scope, ExilePlayPermissionScope::CastCard);
        assert_eq!(
            permission.cast_cost,
            crate::state::ExilePermissionCastCost::AlternativeManaCost(ManaCost::default())
        );
        assert!(engine
            .special_cast_permission(original_controller)
            .is_none());

        let action =
            crate::engine::legal_actions::legal_zone_cast_actions(&engine, copied_controller)
                .into_iter()
                .find(|action| {
                    action.object_id == exiled.object_id
                        && action.zone_change_generation == exiled.zone_change_generation
                })
                .expect("the copied controller's action is bound to the same Exile incarnation");
        assert_eq!(action.source_zone, rv1::CastSourceZone::Exile as i32);
        assert_eq!(action.casting_permission_id, Some(permission.group_id));
        assert_eq!(action.cast_method, rv1::CastMethod::Permission as i32);
        assert_eq!(action.cost, ManaCost::default().to_string());
        assert!(!crate::engine::legal_actions::legal_zone_cast_actions(
            &engine,
            original_controller
        )
        .iter()
        .any(|action| {
            action.object_id == exiled.object_id
                && action.zone_change_generation == exiled.zone_change_generation
        }));

        let physical_opt = &engine.state.objects[&exiled.object_id];
        assert_eq!(physical_opt.zone, Zone::Exile);
        assert_eq!(physical_opt.owner, exiled_owner);
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&exiled.object_id)
                .copied(),
            Some(exiled.zone_change_generation)
        );
    }
}
