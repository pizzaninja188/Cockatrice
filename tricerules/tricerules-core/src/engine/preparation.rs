//! CR 722: a preparation designation owns one persistent, noncard copy in exile.
use super::*;
use crate::state::{
    ExilePermissionCastCost, ExilePlayPermissionOrigin, PreparedExileCopyRef,
    PreparedPermanentDesignation,
};

impl GameEngine {
    pub(super) fn preparation_view(&self, id: ObjectId) -> Option<rv1::PreparationState> {
        self.prepared_designation_view(id)
            .map(|designation| rv1::PreparationState {
                copy_object_id: designation.active_exile_copy.map(|copy| copy.object_id),
                copy_zone_change_generation: designation
                    .active_exile_copy
                    .map(|copy| copy.zone_change_generation),
            })
    }

    /// Derive the public designation and only its currently linked, exact exile incarnation.
    pub(super) fn prepared_designation_view(
        &self,
        source_id: ObjectId,
    ) -> Option<PreparedPermanentDesignation> {
        let mut designation = *self.state.prepared_permanents.get(&source_id)?;
        let source = self.state.objects.get(&source_id)?;
        let source_generation = self
            .state
            .zone_change_generation
            .get(&source_id)
            .copied()
            .unwrap_or(0);
        if source.zone != Zone::Battlefield
            || source_generation != designation.source_zone_change_generation
        {
            return None;
        }
        designation.active_exile_copy = designation.active_exile_copy.filter(|copy_ref| {
            source.phase_status.is_none()
                && self
                    .state
                    .objects
                    .get(&copy_ref.object_id)
                    .is_some_and(|copy| copy.zone == Zone::Exile)
                && self
                    .state
                    .zone_change_generation
                    .get(&copy_ref.object_id)
                    .copied()
                    .unwrap_or(0)
                    == copy_ref.zone_change_generation
                && self
                    .state
                    .prepare_spell_sources
                    .get(&copy_ref.object_id)
                    .is_some_and(|source| {
                        source.object_id == source_id
                            && source.zone_change_generation == source_generation
                    })
        });
        Some(designation)
    }

    pub(super) fn prepare_permanent(&mut self, source_id: ObjectId) {
        if !self
            .state
            .objects
            .get(&source_id)
            .is_some_and(|source| source.zone == Zone::Battlefield && source.phase_status.is_none())
        {
            return;
        }
        let source_generation = self
            .state
            .zone_change_generation
            .get(&source_id)
            .copied()
            .unwrap_or(0);
        // CR 722.3a: a present designation means this permanent already is prepared, even if
        // its linked exile copy was displaced and cleaned up. Only a new battlefield
        // incarnation (whose zone move ended the old designation) can gain preparation again.
        if self.state.prepared_permanents.contains_key(&source_id) {
            return;
        }
        self.create_prepared_exile_copy(source_id, source_generation);
    }

    /// CR 722.3c: phase-in recreates the copy for an existing designation whose active copy was
    /// retired on phase-out. Ordinary Prepare effects must not call this path.
    pub(super) fn recreate_prepared_exile_copy_on_phase_in(&mut self, source_id: ObjectId) {
        let source_generation = self
            .state
            .zone_change_generation
            .get(&source_id)
            .copied()
            .unwrap_or(0);
        let phase_in_recreation_is_due = self
            .state
            .prepared_permanents
            .get(&source_id)
            .is_some_and(|designation| {
                designation.source_zone_change_generation == source_generation
                    && designation.active_exile_copy.is_none()
            });
        if phase_in_recreation_is_due {
            self.create_prepared_exile_copy(source_id, source_generation);
        }
    }

    fn create_prepared_exile_copy(&mut self, source_id: ObjectId, source_generation: u64) {
        let Some(source) = self
            .state
            .objects
            .get(&source_id)
            .filter(|source| source.zone == Zone::Battlefield && source.phase_status.is_none())
        else {
            return;
        };
        let source_generation_is_current = self
            .state
            .zone_change_generation
            .get(&source_id)
            .copied()
            .unwrap_or(0)
            == source_generation;
        let existing_designation_allows_copy = self
            .state
            .prepared_permanents
            .get(&source_id)
            .is_none_or(|designation| {
                designation.source_zone_change_generation == source_generation
                    && designation.active_exile_copy.is_none()
            });
        if !source_generation_is_current || !existing_designation_allows_copy {
            return;
        }
        let controller = source.controller;
        let Some(values) = self.copiable_values_for(source_id) else {
            return;
        };
        // The inset is part of the copied definition, unaffected by copy exceptions on the base.
        let Some(definition) = self
            .registry
            .get(&values.source_card_id)
            .filter(|definition| {
                definition.layout == Layout::Preparation && values.source_face_index == 0
            })
        else {
            return;
        };
        let face = definition.face(1).expect("validated preparation inset");
        let copy_id = self.state.next_object_id;
        self.state.next_object_id += 1;
        let mut copy = new_object_from_card(copy_id, controller, &definition.id, Zone::Exile, face);
        copy.face_up_index = 1;
        self.state.objects.insert(copy_id, copy);
        self.state.zone_change_generation.insert(copy_id, 0);
        self.state
            .players
            .iter_mut()
            .find(|player| player.id == controller)
            .unwrap()
            .exile
            .push(copy_id);
        let copy_ref = PreparedExileCopyRef {
            object_id: copy_id,
            zone_change_generation: 0,
        };
        self.state
            .prepared_permanents
            .entry(source_id)
            .and_modify(|designation| designation.active_exile_copy = Some(copy_ref))
            .or_insert(PreparedPermanentDesignation {
                source_zone_change_generation: source_generation,
                active_exile_copy: Some(copy_ref),
            });
        self.state.prepare_spell_sources.insert(
            copy_id,
            TriggerObjectRef {
                object_id: source_id,
                zone_change_generation: source_generation,
                controller_at_event: controller,
            },
        );
        let group_id = self.state.next_exile_play_permission_group_id;
        self.state.next_exile_play_permission_group_id += 1;
        self.state
            .active_exile_play_permissions
            .push(ActiveExilePlayPermission {
                group_id,
                player_id: controller,
                source_label: values.face.name,
                object_id: copy_id,
                zone_change_generation: 0,
                scope: ExilePlayPermissionScope::CastFace(1),
                max_face_mana_value: None,
                cast_cost: ExilePermissionCastCost::PrintedManaCost,
                origin: ExilePlayPermissionOrigin::Preparation {
                    source_object_id: source_id,
                    source_generation,
                },
                available_after_turn_instance: None,
                expires_at_cleanup_turn_instance: None,
            });
    }

    pub(super) fn exile_permission_controller(
        &self,
        permission: &ActiveExilePlayPermission,
    ) -> Option<PlayerId> {
        match permission.origin {
            ExilePlayPermissionOrigin::Preparation {
                source_object_id,
                source_generation,
            } => self
                .state
                .objects
                .get(&source_object_id)
                .filter(|source| {
                    source.zone == Zone::Battlefield
                        && source.phase_status.is_none()
                        && self
                            .state
                            .zone_change_generation
                            .get(&source_object_id)
                            .copied()
                            .unwrap_or(0)
                            == source_generation
                        && self
                            .state
                            .prepared_permanents
                            .get(&source_object_id)
                            .is_some_and(|designation| {
                                designation.source_zone_change_generation == source_generation
                                    && designation.active_exile_copy
                                        == Some(PreparedExileCopyRef {
                                            object_id: permission.object_id,
                                            zone_change_generation: permission
                                                .zone_change_generation,
                                        })
                            })
                        && self
                            .state
                            .objects
                            .get(&permission.object_id)
                            .is_some_and(|copy| copy.zone == Zone::Exile)
                        && self
                            .state
                            .zone_change_generation
                            .get(&permission.object_id)
                            .copied()
                            .unwrap_or(0)
                            == permission.zone_change_generation
                })
                .map(|source| source.controller),
            _ => Some(permission.player_id),
        }
    }
}

/// Retire the exact active exile copy while preserving the source's prepared designation.
pub(super) fn retire_active_prepared_exile_copy(
    state: &mut GameState,
    source_id: ObjectId,
    source_generation: u64,
) -> Option<PreparedExileCopyRef> {
    let copy_ref = {
        let designation = state.prepared_permanents.get_mut(&source_id)?;
        if designation.source_zone_change_generation != source_generation {
            return None;
        }
        designation.active_exile_copy.take()?
    };
    let exact_exile_copy = state.objects.get(&copy_ref.object_id).is_some_and(|copy| {
        copy.zone == Zone::Exile
            && state
                .zone_change_generation
                .get(&copy_ref.object_id)
                .copied()
                .unwrap_or(0)
                == copy_ref.zone_change_generation
    });
    let exact_link = state
        .prepare_spell_sources
        .get(&copy_ref.object_id)
        .is_some_and(|source| {
            source.object_id == source_id && source.zone_change_generation == source_generation
        });
    if exact_exile_copy {
        state.objects.remove(&copy_ref.object_id);
        state.zone_change_generation.remove(&copy_ref.object_id);
        for player in &mut state.players {
            player.exile.retain(|&id| id != copy_ref.object_id);
        }
    }
    if exact_exile_copy && exact_link {
        state.prepare_spell_sources.remove(&copy_ref.object_id);
    }
    state.active_exile_play_permissions.retain(|permission| {
        !(permission.object_id == copy_ref.object_id
            && permission.zone_change_generation == copy_ref.zone_change_generation
            && permission.origin
                == (ExilePlayPermissionOrigin::Preparation {
                    source_object_id: source_id,
                    source_generation,
                }))
    });
    Some(copy_ref)
}

/// End only the designation belonging to this source incarnation (CR 400.7, 722.3c).
pub(super) fn end_prepared_designation(
    state: &mut GameState,
    source_id: ObjectId,
    source_generation: u64,
) {
    if state
        .prepared_permanents
        .get(&source_id)
        .is_none_or(|designation| designation.source_zone_change_generation != source_generation)
    {
        return;
    }
    retire_active_prepared_exile_copy(state, source_id, source_generation);
    state.prepared_permanents.remove(&source_id);
}

pub(super) fn unprepare_permanent(state: &mut GameState, source_id: ObjectId) {
    let source_generation = state
        .zone_change_generation
        .get(&source_id)
        .copied()
        .unwrap_or(0);
    end_prepared_designation(state, source_id, source_generation);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tricerules_cards::primitives::CounterKind;

    fn setup() -> GameEngine {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            180_101,
            &[3, 11],
            20,
            Some(vec![vec!["forest".into(); 20]; 2]),
            true,
        )
        .unwrap();
        // Admission is two-player; the rules machinery remains player-set generic.
        engine.state.players.push(PlayerState::new(299, 20));
        engine.state.next_object_id = 300;
        engine.state.turn_step = TurnStep::Main1;
        engine
    }

    fn source(engine: &mut GameEngine, seat: usize) -> ObjectId {
        let id = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        let controller = engine.state.players[seat].id;
        let def = engine
            .registry
            .get("infirmary_healer_stream_of_life")
            .unwrap();
        engine.state.objects.insert(
            id,
            new_object_from_card(
                id,
                controller,
                &def.id,
                Zone::Battlefield,
                def.primary_face(),
            ),
        );
        engine.state.players[seat].battlefield.push(id);
        engine.state.zone_change_generation.insert(id, 1);
        engine.prepare_permanent(id);
        id
    }

    fn active_copy(engine: &GameEngine, source_id: ObjectId) -> ObjectId {
        engine.state.prepared_permanents[&source_id]
            .active_exile_copy
            .expect("prepared source has an active exile copy")
            .object_id
    }

    #[test]
    fn phased_prepared_permanent_keeps_designation_and_recreates_a_fresh_copy_at_its_untap() {
        let mut engine = setup();
        let source_id = source(&mut engine, 0);
        let source_generation = engine.state.zone_change_generation[&source_id];
        let old_copy = active_copy(&engine, source_id);
        let old_copy_generation = engine.state.zone_change_generation[&old_copy];
        let old_permission = engine
            .state
            .active_exile_play_permissions
            .iter()
            .find(|permission| permission.object_id == old_copy)
            .unwrap()
            .clone();
        {
            let object = engine.state.objects.get_mut(&source_id).unwrap();
            object.tapped = true;
            object.counters.insert(CounterKind::PlusOnePlusOne, 2);
        }

        assert!(engine
            .phase_out_chosen_creature(source_id, 11, &mut Vec::new())
            .unwrap());
        let designation = engine.state.prepared_permanents[&source_id];
        assert_eq!(designation.source_zone_change_generation, source_generation);
        assert_eq!(designation.active_exile_copy, None);
        assert_eq!(engine.state.objects[&source_id].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.zone_change_generation[&source_id],
            source_generation
        );
        assert!(engine.state.objects[&source_id].phase_status.is_some());
        assert!(engine.state.objects[&source_id].tapped);
        assert_eq!(
            engine.state.objects[&source_id].counter_count(CounterKind::PlusOnePlusOne),
            2
        );
        assert!(!engine.state.objects.contains_key(&old_copy));
        assert!(!engine
            .state
            .active_exile_play_permissions
            .iter()
            .any(|permission| permission.object_id == old_copy));
        assert_eq!(engine.exile_permission_controller(&old_permission), None);
        let out_view = engine.preparation_view(source_id).unwrap();
        assert_eq!(out_view.copy_object_id, None);
        assert_eq!(out_view.copy_zone_change_generation, None);

        for expected_active in [11, 299] {
            engine.finish_cleanup_roll_new_turn(Vec::new()).unwrap();
            assert_eq!(engine.state.active_player_id(), expected_active);
            assert!(engine.state.objects[&source_id].phase_status.is_some());
            assert_eq!(
                engine.state.prepared_permanents[&source_id].active_exile_copy,
                None
            );
        }
        engine.finish_cleanup_roll_new_turn(Vec::new()).unwrap();
        assert_eq!(engine.state.active_player_id(), 3);
        assert!(engine.state.objects[&source_id].phase_status.is_none());
        assert_eq!(
            engine.state.zone_change_generation[&source_id],
            source_generation
        );
        assert_eq!(
            engine.state.objects[&source_id].counter_count(CounterKind::PlusOnePlusOne),
            2
        );
        assert!(!engine.state.objects[&source_id].tapped);
        let new_copy = active_copy(&engine, source_id);
        assert_ne!(new_copy, old_copy);
        assert!(!engine.state.objects.contains_key(&old_copy));
        assert_eq!(engine.state.zone_change_generation[&new_copy], 0);
        assert_eq!(old_copy_generation, 0);
        let in_view = engine.preparation_view(source_id).unwrap();
        assert_eq!(in_view.copy_object_id, Some(new_copy));
        assert_eq!(in_view.copy_zone_change_generation, Some(0));
        assert_eq!(
            engine
                .state
                .active_exile_play_permissions
                .iter()
                .find(|permission| { permission.object_id == new_copy })
                .and_then(|permission| engine.exile_permission_controller(permission)),
            Some(3)
        );
    }

    #[test]
    fn already_cast_prepared_copy_remains_independent_when_its_source_phases_out() {
        let mut engine = setup();
        let source_id = source(&mut engine, 0);
        let copy_id = active_copy(&engine, source_id);
        let copy_generation = engine.state.zone_change_generation[&copy_id];
        let permission_id = engine
            .state
            .active_exile_play_permissions
            .iter()
            .find(|permission| permission.object_id == copy_id)
            .expect("prepared copy has its cast permission")
            .group_id;
        engine.state.players[0].mana_pool.green = 3;

        let cast = rv1::CastSpell {
            cast_method: rv1::CastMethod::Normal as i32,
            casting_permission_id: Some(permission_id),
            face_index: 1,
            x_value: 2,
            source: Some(rv1::CastSource {
                expected_zone_change_generation: Some(copy_generation),
                location: Some(rv1::cast_source::Location::ExileObjectId(copy_id)),
            }),
            targets: vec![rv1::TargetRef {
                object_id: 11,
                group_index: 0,
                kind: rv1::TargetRefKind::Player as i32,
                ..Default::default()
            }],
            ..Default::default()
        };
        engine
            .cast_spell(3, &cast)
            .expect("cast the prepared Stream of Life copy from exile");
        assert_eq!(engine.state.objects[&copy_id].zone, Zone::Stack);
        assert!(!engine.state.prepared_permanents.contains_key(&source_id));

        assert!(engine
            .phase_out_chosen_creature(source_id, 11, &mut Vec::new())
            .expect("the opponent phases out the prepared source"));
        assert!(engine.state.objects[&source_id].phase_status.is_some());
        assert_eq!(engine.state.objects[&copy_id].zone, Zone::Stack);
        assert_eq!(engine.state.stack.last().unwrap().id, copy_id);
        assert_eq!(engine.state.stack.last().unwrap().controller, 3);

        while !engine.state.stack.is_empty() && engine.state.pending_resolution.is_none() {
            let player = engine.state.priority_player_id();
            engine
                .apply_scenario_command(
                    player,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                    },
                )
                .expect("pass priority to resolve the independent cast copy");
        }
        assert_eq!(engine.state.players[1].life, 22);
        assert!(engine.state.objects[&source_id].phase_status.is_some());
    }

    #[test]
    fn preparation_permissions_follow_current_controller_and_distinct_sources() {
        let mut engine = setup();
        let first = source(&mut engine, 0);
        let second = source(&mut engine, 0);
        let first_copy = active_copy(&engine, first);
        let second_copy = active_copy(&engine, second);
        assert_ne!(first_copy, second_copy);
        let permission = engine
            .state
            .active_exile_play_permissions
            .iter()
            .find(|p| p.object_id == first_copy)
            .unwrap()
            .clone();
        assert_eq!(engine.exile_permission_controller(&permission), Some(3));
        engine.state.objects.get_mut(&first).unwrap().controller = 299;
        engine
            .state
            .objects
            .get_mut(&first)
            .unwrap()
            .base_controller = 299;
        engine.state.players[0]
            .battlefield
            .retain(|id| *id != first);
        engine.state.players[2].battlefield.push(first);
        engine.state.priority_idx = 2;
        engine.state.active_player_idx = 2;
        assert_eq!(engine.exile_permission_controller(&permission), Some(299));
        let batch = engine.initial_response_batch();
        let offered = &batch.legal_by_player[&299].zone_cast_actions;
        assert!(offered.iter().any(|a| a.object_id == first_copy
            && a.preparation_source
                .as_ref()
                .is_some_and(|s| s.object_id == first && s.zone_change_generation == 1)));
        assert!(!offered.iter().any(|a| a.object_id == second_copy));
        engine.state.zone_change_generation.insert(first, 2);
        assert_eq!(engine.exile_permission_controller(&permission), None);
    }

    #[test]
    fn preparation_survives_ability_loss_face_down_and_copy_changes_with_captured_inset() {
        let mut engine = setup();
        let id = source(&mut engine, 0);
        let copy = active_copy(&engine, id);
        let mut values = engine.copiable_values_for(id).unwrap();
        values.face.static_abilities.clear();
        values.face.types = vec!["Artifact".into()];
        engine.state.objects.get_mut(&id).unwrap().copiable_values = Some(values);
        engine.prepare_permanent(id);
        assert_eq!(active_copy(&engine, id), copy);
        engine.state.objects.get_mut(&id).unwrap().face_down = true;
        engine.prepare_permanent(id);
        assert_eq!(active_copy(&engine, id), copy);
        engine.state.objects.get_mut(&id).unwrap().face_down = false;
        let bear = engine.registry.get("grizzly_bears").unwrap();
        engine.state.objects.get_mut(&id).unwrap().copiable_values = Some(CopiableValues {
            prototyped: false,
            source_card_id: bear.id.clone(),
            source_face_index: 0,
            face: bear.primary_face().clone(),
            room_faces: None,
            display_name: bear.name.clone(),
        });
        assert_eq!(
            engine.state.objects[&copy].card_id,
            "infirmary_healer_stream_of_life"
        );
        assert_eq!(active_copy(&engine, id), copy);
        unprepare_permanent(&mut engine.state, id);
        engine.prepare_permanent(id);
        assert!(
            !engine.state.prepared_permanents.contains_key(&id),
            "a bear has no inset to prepare"
        );
    }

    #[test]
    fn preparation_leave_reentry_and_displaced_copy_cleanup_are_generation_bound() {
        let mut engine = setup();
        let id = source(&mut engine, 0);
        let first_copy = active_copy(&engine, id);
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            id,
            Zone::Exile,
            None,
        )
        .unwrap();
        assert!(!engine.state.objects.contains_key(&first_copy));
        assert!(!engine.state.prepared_permanents.contains_key(&id));
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            id,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        engine.prepare_permanent(id);
        let second_copy = active_copy(&engine, id);
        assert_ne!(first_copy, second_copy);
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            second_copy,
            Zone::Hand,
            None,
        )
        .unwrap();
        engine.apply_sbas(&mut Vec::new()).unwrap();
        assert!(!engine.state.objects.contains_key(&second_copy));
        assert!(!engine
            .state
            .players
            .iter()
            .any(|p| p.hand.contains(&second_copy)));
        assert!(
            engine.state.prepared_permanents.contains_key(&id),
            "the missing copy does not remove the designation"
        );
        assert!(engine.initial_response_batch().legal_by_player[&3]
            .zone_cast_actions
            .is_empty());
    }

    #[test]
    fn preparation_view_clears_a_displaced_copy_and_refreshes_the_battlefield_snapshot() {
        let mut engine = setup();
        let source_id = source(&mut engine, 0);
        let copy_id = active_copy(&engine, source_id);
        engine.ev_zone_view_sync();

        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            copy_id,
            Zone::Hand,
            None,
        )
        .unwrap();
        engine.apply_sbas(&mut Vec::new()).unwrap();

        let designation = engine.state.prepared_permanents[&source_id];
        assert_eq!(designation.source_zone_change_generation, 1);
        assert_eq!(designation.active_exile_copy, None);
        assert!(!engine.state.objects.contains_key(&copy_id));
        let event = engine.ev_zone_view_sync_tracked();
        let Some(rv1::ruled_event::Ev::ZoneView(zone_view)) = &event.ev else {
            panic!("zone view");
        };
        assert!(!zone_view.battlefields_unchanged);
        let source_view = zone_view.per_player[0]
            .battlefield_objects
            .iter()
            .find(|object| object.object_id == source_id)
            .expect("prepared source remains in the battlefield view");
        let preparation = source_view
            .preparation
            .as_ref()
            .expect("prepared designation remains public");
        assert_eq!(preparation.copy_object_id, None);
        assert_eq!(preparation.copy_zone_change_generation, None);

        let next_object_id_before_reprepare = engine.state.next_object_id;
        let permissions_before_reprepare = engine.state.active_exile_play_permissions.len();
        engine.prepare_permanent(source_id);
        assert_eq!(
            engine.state.prepared_permanents[&source_id].active_exile_copy,
            None,
            "CR 722.3a does not let an ordinary Prepare effect prepare an already-prepared permanent again"
        );
        assert_eq!(
            engine.state.next_object_id, next_object_id_before_reprepare,
            "an ordinary Prepare effect does not allocate a replacement inset"
        );
        assert_eq!(
            engine.state.active_exile_play_permissions.len(),
            permissions_before_reprepare,
            "an ordinary Prepare effect does not restore a cast permission"
        );

        assert!(engine
            .phase_out_chosen_creature(source_id, 11, &mut Vec::new())
            .expect("phase out the still-prepared permanent"));
        for expected_active in [11, 299, 3] {
            engine
                .finish_cleanup_roll_new_turn(Vec::new())
                .expect("advance to the prepared source's untap");
            assert_eq!(engine.state.active_player_id(), expected_active);
            assert_eq!(
                engine.state.objects[&source_id].phase_status.is_some(),
                expected_active != 3,
                "only the prepared source's controller phases it in"
            );
        }
        let phase_in_copy = active_copy(&engine, source_id);
        assert_ne!(phase_in_copy, copy_id);
        assert_eq!(engine.state.zone_change_generation[&phase_in_copy], 0);
    }

    #[test]
    fn preparation_physical_card_play_permission_never_offers_the_inset() {
        let mut engine = setup();
        let id = source(&mut engine, 0);
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            id,
            Zone::Exile,
            None,
        )
        .unwrap();
        let generation = engine.state.zone_change_generation[&id];
        engine
            .state
            .active_exile_play_permissions
            .push(ActiveExilePlayPermission {
                group_id: 90,
                player_id: 3,
                source_label: "play the exiled card".into(),
                object_id: id,
                zone_change_generation: generation,
                scope: ExilePlayPermissionScope::PlayCard,
                max_face_mana_value: None,
                cast_cost: ExilePermissionCastCost::PrintedManaCost,
                origin: ExilePlayPermissionOrigin::Effect,
                available_after_turn_instance: None,
                expires_at_cleanup_turn_instance: None,
            });
        let batch = engine.initial_response_batch();
        let faces: Vec<_> = batch.legal_by_player[&3]
            .zone_cast_actions
            .iter()
            .filter(|a| a.object_id == id)
            .map(|a| a.face_index)
            .collect();
        assert_eq!(faces, vec![0]);
    }

    #[test]
    fn preparation_copiable_inset_ignores_base_exceptions_but_not_face_down_values() {
        let mut engine = setup();
        let original = source(&mut engine, 0);
        let clone = source(&mut engine, 1);
        unprepare_permanent(&mut engine.state, clone);
        let mut values = engine.copiable_values_for(original).unwrap();
        values.face.power = Some(99);
        values.face.static_abilities.clear();
        engine
            .state
            .objects
            .get_mut(&clone)
            .unwrap()
            .copiable_values = Some(values);
        assert!(
            !engine.state.prepared_permanents.contains_key(&clone),
            "copy values do not copy prepared designation"
        );
        engine.prepare_permanent(clone);
        let copy = active_copy(&engine, clone);
        assert_eq!(engine.state.objects[&copy].face_up_index, 1);
        assert_eq!(engine.state.objects[&copy].power, None);
        unprepare_permanent(&mut engine.state, clone);
        engine.state.objects.get_mut(&clone).unwrap().face_down = true;
        engine.prepare_permanent(clone);
        assert!(!engine.state.prepared_permanents.contains_key(&clone));
    }
}
