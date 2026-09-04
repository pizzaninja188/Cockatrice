//! CR 702.185: Warp's cast receipt, delayed exile, and owner casting permission.
//! Shared by Knight Luminary, Weftblade Enhancer, and Perigee Beckoner.

use super::presentation::{
    stack_child_presentation_ref, PresentationPath, StackPresentationSource,
};
use super::*;

impl GameEngine {
    pub(super) fn register_warp_entry(&mut self, item: &StackItem, object_id: ObjectId) {
        if item.cast_method != SpellCastMethod::Warp {
            return;
        }
        let Some(object) = self
            .state
            .objects
            .get(&object_id)
            .filter(|o| o.zone == Zone::Battlefield)
        else {
            return;
        };
        let watched = TriggerObjectRef {
            object_id,
            zone_change_generation: self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
            controller_at_event: object.controller,
        };
        self.state
            .warped_permanent_incarnations
            .insert((object_id, watched.zone_change_generation));
        let face = self
            .registry
            .get(&item.card_id)
            .and_then(|card| card.face(item.face_index));
        let card_name = face.map(|face| face.name.clone()).unwrap_or_default();
        let ability = TriggeredAbilityDef {
            ability_id: tricerules_cards::AbilityId::new("warp_exile")
                .expect("intrinsic ability id"),
            presentation: tricerules_cards::AbilityPresentation::Fallback,
            trigger: TriggerCondition::AtBeginningOfNextEndStep,
            effect: vec![SpellEffectKind::ExileWarpedObject],
            modal: None,
            targeting: None,
            may: false,
            intervening_if: None,
            triggers_only_once: false,
            max_triggers_per_turn: None,
        };
        let ability_text = ability.fallback_text(&card_name);
        let presentation = stack_child_presentation_ref(
            self.registry,
            &item.card_id,
            item.face_index,
            StackPresentationSource::PhysicalSpell,
            PresentationPath::Ability(&ability.ability_id),
            &ability.presentation,
            ability_text,
        );
        self.state.active_event_observers.push(ActiveEventObserver {
            watched,
            matcher: EventObserverMatcher::AtBeginningOfNextEndStep,
            payload: EventObserverPayload::StageDelayedTrigger(Box::new(DelayedTriggerPayload {
                source: watched,
                controller: item.controller,
                card_id: item.card_id.clone(),
                card_name,
                source_face_index: item.face_index,
                presentation,
                ability,
            })),
        });
    }

    pub(super) fn resolve_warp_exile(
        &mut self,
        item: &StackItem,
        events: &mut Vec<RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(watched) = item.trigger_context.observed_object else {
            return Err(EngineError::Illegal("Warp delayed object missing"));
        };
        let Some(object) = self.state.objects.get(&watched.object_id) else {
            return Ok(());
        };
        if object.zone != Zone::Battlefield
            || self
                .state
                .zone_change_generation
                .get(&object.id)
                .copied()
                .unwrap_or(0)
                != watched.zone_change_generation
        {
            return Ok(());
        }
        let owner = object.owner;
        let is_token = object.is_token();
        let snapshot = self.snapshot_zone_event();
        move_object_to_zone(
            &mut self.state,
            self.registry,
            watched.object_id,
            Zone::Exile,
            None,
        )?;
        events.push(permanent_moved_event(
            &self.state,
            watched.object_id,
            owner,
            rv1::permanent_moved::Destination::Exile,
        ));
        self.fire_zone_triggers(snapshot, vec![]);
        if !is_token
            && self
                .state
                .objects
                .get(&watched.object_id)
                .is_some_and(|o| o.zone == Zone::Exile)
        {
            let label = format!(
                "Warp — {}",
                self.registry
                    .get(&item.card_id)
                    .map(|c| c.name.as_str())
                    .unwrap_or("card")
            );
            let group = self.grant_exile_play_permission(
                owner,
                watched.object_id,
                &label,
                crate::state::ExilePlayPermissionGrant {
                    scope: ExilePlayPermissionScope::CastCard,
                    cast_cost: crate::state::ExilePermissionCastCost::PrintedManaCost,
                    origin: crate::state::ExilePlayPermissionOrigin::Warp,
                    available_after_turn_instance: Some(self.state.turn_instance),
                    until_end_of_next_turn: false,
                },
            )?;
            debug_assert!(self
                .state
                .active_exile_play_permissions
                .iter()
                .any(|permission| permission.group_id == group));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    fn engine() -> GameEngine {
        let registry = CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/forest.ron"),
                r#"(id: "warp_test", name: "Warp Test", face_id: "warp_test", mana_cost: "{3}{W}",
                warp_cost: Some("{1}{W}"), types: ["Creature"], power: 3, toughness: 2)"#,
                r#"(id: "copy_test", name: "Copy Test", face_id: "copy_test", mana_cost: "{U}", types: ["Instant"],
                spell_effect: [CopyTargetSpell()])"#,
            ],
            &[],
        )
        .unwrap();
        let mut engine = GameEngine::new(
            148,
            &[0, 1],
            20,
            Some(vec![vec!["forest".into(); 20]; 2]),
            true,
        )
        .unwrap();
        engine.registry = Box::leak(Box::new(registry));
        engine.state.turn_step = TurnStep::Main1;
        engine.state.priority_idx = 0;
        engine
    }

    fn add(engine: &mut GameEngine, player: usize, card: &str) -> ObjectId {
        let oid = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        let face = engine.registry.get(card).unwrap().primary_face();
        let object =
            new_object_from_card(oid, engine.state.players[player].id, card, Zone::Hand, face);
        engine.state.objects.insert(oid, object);
        engine.state.players[player].hand.push(oid);
        oid
    }

    fn pass(engine: &mut GameEngine) {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(
                player,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
    }

    fn resolve(engine: &mut GameEngine) {
        for _ in 0..30 {
            if let Some(pending) = engine.state.pending_trigger_order.as_ref() {
                let player = pending.deciding_player;
                let trigger_object_id = pending.candidates[0].object_id;
                engine
                    .apply_command(
                        player,
                        &RuledCommand {
                            cmd: Some(rv1::ruled_command::Cmd::SubmitTriggerOrder(
                                rv1::SubmitTriggerOrder { trigger_object_id },
                            )),
                        },
                    )
                    .unwrap();
                continue;
            }
            if engine.state.stack.is_empty() {
                return;
            }
            pass(engine);
        }
        panic!("resolution stalled");
    }

    fn cast_warp(engine: &mut GameEngine) -> ObjectId {
        let oid = add(engine, 0, "warp_test");
        let slot = engine.state.players[0].hand.len() as u32 - 1;
        engine.state.players[0].mana_pool.white = 1;
        engine.state.players[0].mana_pool.colorless = 1;
        engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                        cast_method: rv1::CastMethod::Warp as i32,
                        source: Some(rv1::CastSource {
                            expected_zone_change_generation: None,
                            location: Some(rv1::cast_source::Location::HandIndex(slot)),
                        }),
                        ..Default::default()
                    })),
                },
            )
            .unwrap();
        oid
    }

    #[test]
    fn issue_148_warp_exiles_at_next_end_step_and_waits_for_a_later_turn() {
        let mut engine = engine();
        let oid = cast_warp(&mut engine);
        resolve(&mut engine);
        assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
        engine.state.turn_step = TurnStep::Main2;
        pass(&mut engine);
        pass(&mut engine);
        assert_eq!(engine.state.turn_step, TurnStep::EndStep);
        assert_eq!(
            engine.state.stack.len(),
            1,
            "Warp must use a respondable delayed trigger"
        );
        let delayed_presentation = engine.state.stack_presentations[&engine.state.stack[0].id]
            .primary
            .as_ref()
            .expect("displayed Warp delayed trigger keeps presentation identity");
        assert_eq!(delayed_presentation.card_id, "warp_test");
        assert_eq!(delayed_presentation.face_id, "warp_test");
        assert_eq!(
            delayed_presentation
                .path
                .iter()
                .map(|component| component.id.as_str())
                .collect::<Vec<_>>(),
            vec!["spell", "warp_exile"]
        );
        assert_eq!(
            delayed_presentation.path[1].kind,
            rv1::PresentationPathKind::Ability as i32
        );
        resolve(&mut engine);
        assert_eq!(engine.state.objects[&oid].zone, Zone::Exile);
        assert_eq!(engine.state.active_exile_play_permissions.len(), 1);
        assert!(engine.initial_response_batch().legal_by_player[&0]
            .zone_cast_actions
            .is_empty());
        engine.state.turn_instance += 1;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.priority_idx = 0;
        let batch = engine.initial_response_batch();
        let action = batch.legal_by_player[&0]
            .zone_cast_actions
            .iter()
            .find(|a| a.object_id == oid)
            .unwrap();
        assert_eq!(action.cast_method, rv1::CastMethod::Normal as i32);
        assert_eq!(action.cost, "{3}{W}");
        assert!(action.casting_permission_id.is_some());
        assert!(batch.legal_by_player[&1].zone_cast_actions.is_empty());
    }

    #[test]
    fn issue_148_stale_exile_cast_is_rejected_even_when_a_new_permission_exists() {
        let mut engine = engine();
        let oid = cast_warp(&mut engine);
        resolve(&mut engine);
        engine.state.turn_step = TurnStep::Main2;
        pass(&mut engine);
        pass(&mut engine);
        resolve(&mut engine);
        engine.state.turn_instance += 1;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.priority_idx = 0;
        let generation = engine.state.zone_change_generation[&oid];
        let permission_id = engine
            .state
            .active_exile_play_permissions
            .iter()
            .find(|permission| permission.object_id == oid)
            .expect("old Warp permission")
            .group_id;
        let command = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                source: Some(rv1::CastSource {
                    location: Some(rv1::cast_source::Location::ExileObjectId(oid)),
                    expected_zone_change_generation: Some(generation),
                }),
                cast_method: rv1::CastMethod::Normal as i32,
                casting_permission_id: Some(permission_id),
                ..Default::default()
            })),
        };
        move_object_to_zone(&mut engine.state, engine.registry, oid, Zone::Hand, None).unwrap();
        move_object_to_zone(&mut engine.state, engine.registry, oid, Zone::Exile, None).unwrap();
        engine
            .grant_exile_play_permission(
                0,
                oid,
                "new permission",
                crate::state::ExilePlayPermissionGrant::printed(
                    ExilePlayPermissionScope::CastCard,
                    false,
                ),
            )
            .unwrap();
        engine.state.players[0].mana_pool.white = 1;
        engine.state.players[0].mana_pool.colorless = 3;
        let before = engine.state.command_index;
        assert!(matches!(
            engine.apply_command(0, &command),
            Err(EngineError::Illegal(_))
        ));
        assert_eq!(engine.state.command_index, before);
        assert_eq!(engine.state.objects[&oid].zone, Zone::Exile);
        assert_eq!(engine.state.players[0].mana_pool.white, 1);
    }

    #[test]
    fn issue_148_copied_warp_enters_as_token_then_exiles_without_permission() {
        let mut engine = engine();
        let original = cast_warp(&mut engine);
        add(&mut engine, 0, "copy_test");
        engine.state.players[0].mana_pool.blue = 1;
        let slot = engine.state.players[0].hand.len() as u32 - 1;
        engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                        source: Some(rv1::CastSource {
                            location: Some(rv1::cast_source::Location::HandIndex(slot)),
                            ..Default::default()
                        }),
                        cast_method: rv1::CastMethod::Normal as i32,
                        targets: vec![rv1::TargetRef {
                            object_id: original,
                            kind: rv1::TargetRefKind::Stack as i32,
                            ..Default::default()
                        }],
                        ..Default::default()
                    })),
                },
            )
            .unwrap();
        pass(&mut engine);
        pass(&mut engine);
        let copy = engine.state.stack.last().unwrap().id;
        assert_ne!(copy, original);
        assert!(engine.state.stack.last().unwrap().is_copy);
        resolve(&mut engine);
        assert_eq!(
            engine.state.turn_history.current.spells_cast, 2,
            "copy is not a cast"
        );
        assert_eq!(
            engine
                .state
                .objects
                .get(&copy)
                .map(|o| (o.zone, o.is_token())),
            Some((Zone::Battlefield, true))
        );
        assert_eq!(engine.state.active_event_observers.len(), 2);
        engine.state.turn_step = TurnStep::Main2;
        pass(&mut engine);
        pass(&mut engine);
        // Same-controller delayed triggers may require their ordinary ordering choice.
        assert!(
            engine.state.pending_triggers.is_empty(),
            "unexpected targeted trigger"
        );
        resolve(&mut engine);
        assert!(!engine.state.objects.contains_key(&copy));
        assert_eq!(engine.state.active_exile_play_permissions.len(), 1);
        assert_eq!(
            engine.state.active_exile_play_permissions[0].object_id,
            original
        );
    }

    fn void(engine: &GameEngine) -> bool {
        engine.condition_holds(
            &GameCondition::Void,
            ConditionContext {
                controller: 0,
                source_object_id: 0,
                source_zone_change: 0,
                resolving_spell_id: None,
                stack_item: None,
                previous_effect_result: None,
            },
        )
    }

    #[test]
    fn issue_148_void_records_casts_and_actual_nonland_departures() {
        let mut engine = engine();
        assert!(!void(&engine));
        let spell = cast_warp(&mut engine);
        assert!(void(&engine), "Warp counts before resolution");
        // Countering does not erase the committed cast occurrence.
        engine.state.stack.clear();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            spell,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        assert!(void(&engine));
        engine.state.turn_history.finish_turn();
        assert!(!void(&engine));
        let land = add(&mut engine, 1, "forest");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            land,
            Zone::Battlefield,
            Some(1),
        )
        .unwrap();
        let snapshot = engine.snapshot_zone_event();
        move_object_to_zone(&mut engine.state, engine.registry, land, Zone::Hand, None).unwrap();
        engine.fire_zone_triggers(snapshot, vec![]);
        assert!(!void(&engine), "a land departure does not satisfy Void");
        let creature = add(&mut engine, 1, "warp_test");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            creature,
            Zone::Battlefield,
            Some(1),
        )
        .unwrap();
        let snapshot = engine.snapshot_zone_event();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            creature,
            Zone::Hand,
            None,
        )
        .unwrap();
        engine.fire_zone_triggers(snapshot, vec![]);
        assert!(
            void(&engine),
            "any player's nonland departure qualifies, without a watcher"
        );
    }

    #[test]
    fn issue_148_delayed_exile_tracks_generation_not_current_controller_or_types() {
        for blink in [false, true] {
            let mut e = engine();
            let oid = cast_warp(&mut e);
            resolve(&mut e);
            if blink {
                move_object_to_zone(&mut e.state, e.registry, oid, Zone::Exile, None).unwrap();
                move_object_to_zone(&mut e.state, e.registry, oid, Zone::Battlefield, Some(1))
                    .unwrap();
            } else {
                e.state.objects.get_mut(&oid).unwrap().controller = 1;
            }
            e.state.turn_step = TurnStep::Main2;
            pass(&mut e);
            pass(&mut e);
            resolve(&mut e);
            assert_eq!(
                e.state.objects[&oid].zone,
                if blink {
                    Zone::Battlefield
                } else {
                    Zone::Exile
                }
            );
            assert_eq!(
                e.state.active_exile_play_permissions.len(),
                usize::from(!blink)
            );
            if !blink {
                assert_eq!(
                    e.state.active_exile_play_permissions[0].player_id, 0,
                    "owner gets permission"
                );
            }
        }
    }

    #[test]
    fn issue_148_entry_after_end_step_trigger_point_waits_for_next_end_step() {
        let mut e = engine();
        let oid = cast_warp(&mut e);
        e.state.turn_step = TurnStep::EndStep;
        resolve(&mut e);
        assert_eq!(e.state.objects[&oid].zone, Zone::Battlefield);
        assert_eq!(e.state.active_event_observers.len(), 1);
        e.state.turn_instance += 1;
        e.state.turn_step = TurnStep::Main2;
        pass(&mut e);
        pass(&mut e);
        resolve(&mut e);
        assert_eq!(e.state.objects[&oid].zone, Zone::Exile);
        assert!(!e.state.active_exile_play_permissions[0].available_on_turn(e.state.turn_instance));
    }

    #[test]
    fn issue_148_failed_warp_does_not_record_void_or_move_the_card() {
        let mut e = engine();
        let oid = add(&mut e, 0, "warp_test");
        let slot = e.state.players[0].hand.len() as u32 - 1;
        let before = e.state.command_index;
        assert!(e
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                        source: Some(rv1::CastSource {
                            location: Some(rv1::cast_source::Location::HandIndex(slot)),
                            ..Default::default()
                        }),
                        cast_method: rv1::CastMethod::Warp as i32,
                        ..Default::default()
                    }))
                }
            )
            .is_err());
        assert_eq!(e.state.command_index, before);
        assert_eq!(e.state.objects[&oid].zone, Zone::Hand);
        assert!(!void(&e));
    }

    #[test]
    fn issue_148_warp_stack_and_battlefield_annotations_are_not_redundant() {
        let mut e = engine();
        let oid = add(&mut e, 0, "warp_test");
        let slot = e.state.players[0].hand.len() as u32 - 1;
        e.state.players[0].mana_pool.white = 1;
        e.state.players[0].mana_pool.colorless = 1;
        let batch = e
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                        source: Some(rv1::CastSource {
                            location: Some(rv1::cast_source::Location::HandIndex(slot)),
                            ..Default::default()
                        }),
                        cast_method: rv1::CastMethod::Warp as i32,
                        ..Default::default()
                    })),
                },
            )
            .unwrap();
        let pushed = batch
            .events
            .iter()
            .find_map(|event| match event.ev.as_ref() {
                Some(rv1::ruled_event::Ev::StackPushed(pushed)) => Some(pushed),
                _ => None,
            })
            .unwrap();
        assert_eq!(pushed.ability_annotation, "Warp");

        resolve(&mut e);
        let view = e
            .initial_response_batch()
            .events
            .into_iter()
            .find_map(|event| match event.ev {
                Some(rv1::ruled_event::Ev::ZoneView(view)) => Some(view),
                _ => None,
            })
            .unwrap();
        let permanent = view
            .per_player
            .iter()
            .flat_map(|player| &player.battlefield_objects)
            .find(|object| object.object_id == oid)
            .unwrap();
        assert_eq!(permanent.rules_annotation_labels, vec!["Warped"]);

        move_object_to_zone(&mut e.state, e.registry, oid, Zone::Exile, None).unwrap();
        move_object_to_zone(&mut e.state, e.registry, oid, Zone::Battlefield, None).unwrap();
        let view = e
            .initial_response_batch()
            .events
            .into_iter()
            .find_map(|event| match event.ev {
                Some(rv1::ruled_event::Ev::ZoneView(view)) => Some(view),
                _ => None,
            })
            .unwrap();
        let returned = view
            .per_player
            .iter()
            .flat_map(|player| &player.battlefield_objects)
            .find(|object| object.object_id == oid)
            .unwrap();
        assert!(!returned
            .rules_annotation_labels
            .iter()
            .any(|label| label == "Warped"));
    }
}
