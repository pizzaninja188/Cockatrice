//! CR 722: a preparation designation owns one persistent, noncard copy in exile.
use super::*;
use crate::state::{ExilePermissionCastCost, ExilePlayPermissionOrigin};

impl GameEngine {
    pub(super) fn preparation_view(&self, id: ObjectId) -> Option<rv1::PreparationState> {
        self.state
            .prepared_permanents
            .get(&id)
            .map(|copy| rv1::PreparationState {
                copy_object_id: *copy,
                copy_zone_change_generation: self
                    .state
                    .zone_change_generation
                    .get(copy)
                    .copied()
                    .unwrap_or(0),
            })
    }

    pub(super) fn prepare_permanent(&mut self, source_id: ObjectId) {
        if self.state.prepared_permanents.contains_key(&source_id) {
            return;
        }
        let Some(source) = self
            .state
            .objects
            .get(&source_id)
            .filter(|source| source.zone == Zone::Battlefield)
        else {
            return;
        };
        let controller = source.controller;
        let source_generation = self
            .state
            .zone_change_generation
            .get(&source_id)
            .copied()
            .unwrap_or(0);
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
        self.state.prepared_permanents.insert(source_id, copy_id);
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
                        && self
                            .state
                            .zone_change_generation
                            .get(&source_object_id)
                            .copied()
                            .unwrap_or(0)
                            == source_generation
                        && self.state.prepared_permanents.get(&source_object_id)
                            == Some(&permission.object_id)
                })
                .map(|source| source.controller),
            _ => Some(permission.player_id),
        }
    }
}

pub(super) fn unprepare_permanent(state: &mut GameState, source_id: ObjectId) {
    let Some(copy_id) = state.prepared_permanents.remove(&source_id) else {
        return;
    };
    if state
        .objects
        .get(&copy_id)
        .is_some_and(|copy| copy.zone == Zone::Exile)
    {
        state.objects.remove(&copy_id);
        state.prepare_spell_sources.remove(&copy_id);
        state.zone_change_generation.remove(&copy_id);
        for player in &mut state.players {
            player.exile.retain(|&id| id != copy_id);
        }
        state
            .active_exile_play_permissions
            .retain(|permission| permission.object_id != copy_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> GameEngine {
        let mut engine = GameEngine::new(
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

    #[test]
    fn preparation_permissions_follow_current_controller_and_distinct_sources() {
        let mut engine = setup();
        let first = source(&mut engine, 0);
        let second = source(&mut engine, 0);
        let first_copy = engine.state.prepared_permanents[&first];
        let second_copy = engine.state.prepared_permanents[&second];
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
        let copy = engine.state.prepared_permanents[&id];
        let mut values = engine.copiable_values_for(id).unwrap();
        values.face.static_abilities.clear();
        values.face.types = vec!["Artifact".into()];
        engine.state.objects.get_mut(&id).unwrap().copiable_values = Some(values);
        engine.prepare_permanent(id);
        assert_eq!(engine.state.prepared_permanents[&id], copy);
        engine.state.objects.get_mut(&id).unwrap().face_down = true;
        engine.prepare_permanent(id);
        assert_eq!(engine.state.prepared_permanents[&id], copy);
        engine.state.objects.get_mut(&id).unwrap().face_down = false;
        let bear = engine.registry.get("grizzly_bears").unwrap();
        engine.state.objects.get_mut(&id).unwrap().copiable_values = Some(CopiableValues {
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
        assert_eq!(engine.state.prepared_permanents[&id], copy);
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
        let first_copy = engine.state.prepared_permanents[&id];
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
        let second_copy = engine.state.prepared_permanents[&id];
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
        let copy = engine.state.prepared_permanents[&clone];
        assert_eq!(engine.state.objects[&copy].face_up_index, 1);
        assert_eq!(engine.state.objects[&copy].power, None);
        unprepare_permanent(&mut engine.state, clone);
        engine.state.objects.get_mut(&clone).unwrap().face_down = true;
        engine.prepare_permanent(clone);
        assert!(!engine.state.prepared_permanents.contains_key(&clone));
    }
}
