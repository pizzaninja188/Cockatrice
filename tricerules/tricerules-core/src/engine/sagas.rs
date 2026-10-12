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
    /// control simultaneously. Stage the complete cohort and its replacement choices before
    /// committing any lore counter or collecting the chapter triggers.
    pub(super) fn perform_precombat_saga_lore_action(
        &mut self,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let active = self.state.active_player_id();
        let Some(index) = self.state.player_idx(active) else {
            return Ok(());
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
        let mut proposals = Vec::new();
        let mut saga_refs = Vec::new();
        for saga in sagas {
            if let Some(proposal) = self.proposed_object_counter_placement(
                active,
                saga,
                [(CounterKind::Lore, 1)],
                super::continuous::CounterPlacementOrigin::TurnBased,
            ) {
                saga_refs.push(TriggerObjectRef {
                    object_id: saga,
                    zone_change_generation: self
                        .state
                        .zone_change_generation
                        .get(&saga)
                        .copied()
                        .unwrap_or(0),
                    controller_at_event: active,
                });
                proposals.push(proposal);
            }
        }
        if proposals.is_empty() {
            return Ok(());
        }
        self.begin_unstacked_counter_placement_batch(
            proposals,
            crate::state::CounterPlacementCompletion::SagaLore {
                active_player: active,
                sagas: saga_refs,
            },
            out,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copied_saga() -> (GameEngine, ObjectId) {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            185_100,
            &[0, 1],
            20,
            None,
            true,
        )
        .expect("new");
        let object_id = copied_saga_into(&mut engine, 0, false);
        (engine, object_id)
    }

    fn copied_saga_into(engine: &mut GameEngine, controller: PlayerId, animate: bool) -> ObjectId {
        let object_id = engine.state.players[controller as usize].hand.remove(0);
        engine.state.players[controller as usize]
            .battlefield
            .push(object_id);
        let face = engine
            .registry
            .get("burn,_burn,_tree_and_fern")
            .expect("Burn")
            .primary_face()
            .clone();
        let mut face = face;
        if animate {
            face.types.push("Creature".to_string());
            face.supertypes.retain(|supertype| supertype != "Legendary");
            face.power = Some(1);
            face.toughness = Some(1);
        }
        face.triggered_abilities.retain_mut(|ability| {
            let TriggerCondition::SagaChapter { chapters } = &mut ability.trigger else {
                return false;
            };
            *chapters = vec![4];
            true
        });
        let object = engine.state.objects.get_mut(&object_id).expect("object");
        object.zone = Zone::Battlefield;
        object.controller = controller;
        object.base_controller = controller;
        object.owner = controller;
        object.card_id = "burn,_burn,_tree_and_fern".into();
        object.copiable_values = Some(CopiableValues {
            prototyped: false,
            source_card_id: "burn,_burn,_tree_and_fern".into(),
            source_face_index: 0,
            display_name: "Burn, Burn, Tree and Fern".into(),
            room_faces: None,
            face,
        });
        object_id
    }

    fn battlefield_card(engine: &mut GameEngine, controller: PlayerId, card_id: &str) -> ObjectId {
        let object_id = engine.state.players[controller as usize].hand.remove(0);
        engine.state.players[controller as usize]
            .battlefield
            .push(object_id);
        let object = engine.state.objects.get_mut(&object_id).expect("object");
        object.zone = Zone::Battlefield;
        object.controller = controller;
        object.base_controller = controller;
        object.owner = controller;
        object.card_id = card_id.into();
        object.copiable_values = None;
        object.face_down = false;
        object_id
    }

    fn laezel_with_two_replacement_instances(engine: &mut GameEngine) {
        let object_id = battlefield_card(engine, 0, "laezel,_vlaakiths_champion");
        let mut face = engine
            .registry
            .get("laezel,_vlaakiths_champion")
            .expect("Lae'zel")
            .primary_face()
            .clone();
        let mut second = face.static_abilities[0].clone();
        second.ability_id = tricerules_card_model::AbilityId::new("static_02").unwrap();
        face.static_abilities.push(second);
        engine
            .state
            .objects
            .get_mut(&object_id)
            .expect("Lae'zel object")
            .copiable_values = Some(CopiableValues {
            prototyped: false,
            source_card_id: "laezel,_vlaakiths_champion".into(),
            source_face_index: 0,
            display_name: "Lae'zel replacement fixture".into(),
            room_faces: None,
            face,
        });
    }

    #[test]
    fn simultaneous_saga_lore_choices_commit_all_recipients_together() {
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            185_101,
            &[0, 1],
            20,
        )
        .expect("new");
        engine.state.opening = None;
        engine.state.priority_idx = 0;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.command_index = 5;
        assert_eq!(engine.state.active_player_id(), 0);
        laezel_with_two_replacement_instances(&mut engine);
        let first = copied_saga_into(&mut engine, 0, true);
        let second = copied_saga_into(&mut engine, 0, true);
        assert_eq!(engine.saga_final_chapter(first), Some(4));
        assert_eq!(engine.saga_final_chapter(second), Some(4));
        let mut out = Vec::new();
        engine.perform_precombat_saga_lore_action(&mut out).unwrap();

        let mut choices = 0;
        while let Some(pending) = engine.state.pending_resolution.as_ref() {
            assert_eq!(
                pending.presentation.choice_kind,
                rv1::ChoiceKind::ReplacementEffect,
                "the unstacked turn-based action parks on counter replacement order"
            );
            assert_eq!(
                engine.state.objects[&first].counter_count(CounterKind::Lore),
                0,
                "no Saga commits while any recipient still needs a replacement choice"
            );
            assert_eq!(
                engine.state.objects[&second].counter_count(CounterKind::Lore),
                0,
                "sibling Saga counters remain staged"
            );
            let candidate = pending.presentation.candidates[0];
            engine
                .apply_command(
                    0,
                    &rv1::RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                            rv1::SubmitResolutionChoice {
                                chosen_object_ids: vec![candidate],
                                ..Default::default()
                            },
                        )),
                    },
                )
                .expect("choose a legal counter replacement");
            choices += 1;
        }

        assert_eq!(choices, 2, "each Saga has two one-shot Lae'zel instances");
        assert_eq!(engine.state.objects[&first].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&second].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.objects[&first].counter_count(CounterKind::Lore),
            3
        );
        assert_eq!(
            engine.state.objects[&second].counter_count(CounterKind::Lore),
            3
        );
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
        engine
            .perform_precombat_saga_lore_action(&mut Vec::new())
            .unwrap();
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
        engine
            .perform_precombat_saga_lore_action(&mut Vec::new())
            .unwrap();
        assert_eq!(
            engine.state.objects[&saga].counter_count(CounterKind::Lore),
            1,
            "type loss suppresses the Saga turn-based action"
        );
    }
}
