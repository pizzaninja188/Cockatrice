//! CR 714 Saga lore turn-based actions and shared chapter metadata.

use super::*;

impl GameEngine {
    pub(super) fn saga_final_chapter(&self, object_id: ObjectId) -> Option<u32> {
        let characteristics = self.characteristics(object_id)?;
        if !characteristics.has_type("Enchantment") || !characteristics.has_type("Saga") {
            return None;
        }
        let object = self.state.objects.get(&object_id)?;
        if object.zone != Zone::Battlefield {
            return None;
        }
        let (card_id, face_index) = self.effective_card_identity(object_id)?;
        self.effective_triggered_abilities(object_id, card_id, face_index)
            .into_iter()
            .filter_map(|(_, ability, _)| match ability.trigger {
                TriggerCondition::SagaChapter { chapters } => chapters.into_iter().max(),
                _ => None,
            })
            .max()
    }

    /// CR 714.3b: after the active player's draw step, add one lore counter to every Saga they
    /// control simultaneously. The resulting counter edges form one trigger-collection batch.
    pub(super) fn perform_precombat_saga_lore_action(&mut self) {
        let active = self.state.active_player_id();
        let Some(index) = self.state.player_idx(active) else {
            return;
        };
        let sagas: Vec<_> = self.state.players[index]
            .battlefield
            .iter()
            .copied()
            .filter(|object_id| {
                self.characteristics(*object_id)
                    .is_some_and(|characteristics| {
                        characteristics.has_type("Enchantment") && characteristics.has_type("Saga")
                    })
            })
            .collect();
        let mut counter_events = Vec::new();
        for saga in sagas {
            if let Some(event) = self.place_counters_with_event(saga, CounterKind::Lore, 1, false) {
                counter_events.push(event);
            }
        }
        self.fire_triggers(&counter_events);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copied_saga() -> (GameEngine, ObjectId) {
        let mut engine = GameEngine::new(185_100, &[0, 1], 20, None, true).expect("new");
        let object_id = engine.state.players[0].hand.remove(0);
        engine.state.players[0].battlefield.push(object_id);
        let face = engine
            .registry
            .get("burn,_burn,_tree_and_fern")
            .expect("Burn")
            .primary_face()
            .clone();
        let object = engine.state.objects.get_mut(&object_id).expect("object");
        object.zone = Zone::Battlefield;
        object.card_id = "burn,_burn,_tree_and_fern".into();
        object.copiable_values = Some(CopiableValues {
            source_card_id: "burn,_burn,_tree_and_fern".into(),
            source_face_index: 0,
            display_name: "Burn, Burn, Tree and Fern".into(),
            room_faces: None,
            face,
        });
        (engine, object_id)
    }

    #[test]
    fn issue_185_copies_inherit_chapters_and_type_or_ability_loss_suppresses_them() {
        let (mut engine, saga) = copied_saga();
        assert_eq!(engine.saga_final_chapter(saga), Some(4));

        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: None,
            affected: AffectedScope::Single(saga),
            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
            trigger_grant_origin: None,
        });
        assert_eq!(engine.saga_final_chapter(saga), None);
        engine.perform_precombat_saga_lore_action();
        assert_eq!(
            engine.state.objects[&saga].counter_count(CounterKind::Lore),
            1,
            "the turn-based action depends on the Saga type, not chapter abilities"
        );

        engine.state.continuous_effects.clear();
        engine
            .state
            .objects
            .get_mut(&saga)
            .expect("Saga")
            .copiable_values
            .as_mut()
            .expect("copied values")
            .face
            .types
            .retain(|card_type| card_type != "Saga");
        assert_eq!(engine.saga_final_chapter(saga), None);
        engine.perform_precombat_saga_lore_action();
        assert_eq!(
            engine.state.objects[&saga].counter_count(CounterKind::Lore),
            1,
            "type loss suppresses the Saga turn-based action"
        );
    }
}
