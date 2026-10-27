use super::*;
use crate::engine::resolution::permanent_moved_event_with_library_position;
use crate::state::{ManifestDreadLibraryRef, PendingManifestDreadGraveyardOrder};

impl GameEngine {
    pub(in crate::engine) fn manifest_dread_library_ref_current(
        &self,
        owner: PlayerId,
        reference: ManifestDreadLibraryRef,
    ) -> bool {
        let Some(player_index) = self.state.player_idx(owner) else {
            return false;
        };
        let player = &self.state.players[player_index];
        !player.has_lost
            && player.library.get(reference.library_position as usize) == Some(&reference.object_id)
            && self
                .state
                .objects
                .get(&reference.object_id)
                .is_some_and(|object| object.zone == Zone::Library && object.owner == owner)
            && self
                .state
                .zone_change_generation
                .get(&reference.object_id)
                .copied()
                .unwrap_or(0)
                == reference.zone_change_generation
    }

    pub(in crate::engine) fn manifest_dread_library_cohort_current(
        &self,
        owner: PlayerId,
        cohort: &[ManifestDreadLibraryRef],
    ) -> bool {
        if cohort.is_empty() {
            return false;
        }
        let mut seen = HashSet::new();
        cohort.iter().all(|reference| {
            seen.insert(reference.object_id)
                && self.manifest_dread_library_ref_current(owner, *reference)
        })
    }

    pub(super) fn finish_manifest_dread(
        &mut self,
        pending: PendingResolution,
        chosen: ObjectId,
    ) -> Result<RuledEventBatch, EngineError> {
        let controller = pending.deciding_player;
        let (stack, looked_at) = match &pending.continuation {
            ResolutionContinuation::ManifestDread { stack, looked_at } => {
                (stack.clone(), looked_at.clone())
            }
            _ => return Err(EngineError::Illegal("manifest continuation missing")),
        };
        let looked_at_ids = looked_at
            .iter()
            .map(|reference| reference.object_id)
            .collect::<Vec<_>>();
        let Some(chosen_reference) = looked_at
            .iter()
            .find(|reference| reference.object_id == chosen)
            .copied()
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("manifest choice is stale"));
        };
        let current_top: Vec<ObjectId> = self
            .state
            .player_idx(controller)
            .map(|idx| {
                self.state.players[idx]
                    .library
                    .iter()
                    .take(looked_at.len())
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
        if current_top != looked_at_ids
            || pending.presentation.candidates != looked_at_ids
            || !self.manifest_dread_library_cohort_current(controller, &looked_at)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("manifest choice is stale"));
        }
        let chosen_position = chosen_reference.library_position;
        self.state
            .objects
            .get_mut(&chosen)
            .ok_or(EngineError::Illegal("manifest card is missing"))?
            .face_down = true;

        let mut events = Vec::new();
        let entry = BattlefieldEntryEvent {
            entry_reveal_receipts: Vec::new(),
            mana_colors_spent_to_cast: Default::default(),
            prepared: false,
            object_id: chosen,
            deciding_player: controller,
            destination_controller: controller,
            battle_protector: None,
            face_index: 0,
            unlock_room_door: None,
            chosen_x: 0,
            cast_by: None,
            cast_cost_receipts: Vec::new(),
            player_life_snapshot: self.player_life_snapshot(),
            tapped: false,
            set_types: None,
            chosen_basic_land_type: None,
            chosen_opponents: Vec::new(),
            chosen_creature_types: Vec::new(),
            entry_counters: BTreeMap::new(),
            entry_counter_placements: Vec::new(),
            entry_counter_replacements_resolved: false,
            entry_modifiers: Vec::new(),
            entry_cause: None,
            attached_to: None,
            pending_copy_candidate: None,
            pending_aura_recipient: None,
            accepted_aura_recipient: None,
            applied_effects: Vec::new(),
        };
        let completion = BattlefieldEntryCompletion::ManifestDread {
            owner: controller,
            library_refs: looked_at.clone(),
        };
        match self.begin_battlefield_entry(
            stack.item.clone(),
            entry,
            completion.clone(),
            &mut events,
        )? {
            super::super::replacement::BattlefieldEntryProgress::Skipped(entry) => {
                self.finish_entry_copy_without_recipient(stack, *entry, completion, events)
            }
            super::super::replacement::BattlefieldEntryProgress::Parked => {
                self.transfer_entry_choice_resume(&stack);
                Ok(finish_with_events(self, events))
            }
            super::super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                let entry = *entry;
                self.commit_battlefield_entry(entry, None, &mut events)?;
                events.push(permanent_moved_event_with_library_position(
                    &self.state,
                    chosen,
                    controller,
                    rv1::permanent_moved::Destination::Battlefield,
                    chosen_position,
                ));
                if let Some(other) = looked_at
                    .iter()
                    .find(|reference| reference.object_id != chosen)
                    .copied()
                {
                    move_object_to_zone(
                        &mut self.state,
                        self.registry,
                        other.object_id,
                        Zone::Graveyard,
                        None,
                    )?;
                    events.push(permanent_moved_event_with_library_position(
                        &self.state,
                        other.object_id,
                        controller,
                        rv1::permanent_moved::Destination::Graveyard,
                        other.library_position,
                    ));
                }
                events.push(ev_log(format!("P{controller} manifests dread.")));
                let previous_result = EffectResult {
                    produced_objects: vec![TriggerObjectRef {
                        object_id: chosen,
                        zone_change_generation: self
                            .state
                            .zone_change_generation
                            .get(&chosen)
                            .copied()
                            .unwrap_or(0),
                        controller_at_event: controller,
                    }],
                    ..EffectResult::default()
                };
                self.complete_parked_stack_with_previous(stack, previous_result, events)
            }
        }
    }

    /// Resolve the exact skipped-entry completion. A single looked-at card has no ordering
    /// decision; two cards stay in their Library zones until their owner chooses graveyard order.
    pub(in crate::engine) fn finish_manifest_dread_skipped_entry(
        &mut self,
        stack: ParkedStackResolution,
        skipped_object_id: ObjectId,
        owner: PlayerId,
        library_refs: Vec<ManifestDreadLibraryRef>,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        // A skipped entry restores only the captured Library incarnation. The same physical
        // ObjectId may already identify a later face-down permanent (CR 400.7).
        if library_refs.iter().any(|reference| {
            reference.object_id == skipped_object_id
                && self.manifest_dread_library_ref_current(owner, *reference)
        }) {
            if let Some(object) = self.state.objects.get_mut(&skipped_object_id) {
                object.face_down = false;
            }
        }
        if library_refs.len() == 1 {
            let reference = library_refs[0];
            if reference.object_id == skipped_object_id
                && reference.library_position == 0
                && self.manifest_dread_library_ref_current(owner, reference)
            {
                move_object_to_zone(
                    &mut self.state,
                    self.registry,
                    reference.object_id,
                    Zone::Graveyard,
                    None,
                )?;
                events.push(permanent_moved_event_with_library_position(
                    &self.state,
                    reference.object_id,
                    owner,
                    rv1::permanent_moved::Destination::Graveyard,
                    reference.library_position,
                ));
            }
            return self.complete_parked_stack_with_previous(
                stack,
                EffectResult::default(),
                events,
            );
        }

        let Ok(cohort): Result<[ManifestDreadLibraryRef; 2], _> = library_refs.try_into() else {
            return self.complete_parked_stack_with_previous(
                stack,
                EffectResult::default(),
                events,
            );
        };
        if !cohort
            .iter()
            .any(|reference| reference.object_id == skipped_object_id)
            || cohort[0].library_position != 0
            || cohort[1].library_position != 1
            || !self.manifest_dread_library_cohort_current(owner, &cohort)
            || self
                .state
                .player_idx(owner)
                .is_none_or(|idx| self.state.players[idx].has_lost)
        {
            return self.complete_parked_stack_with_previous(
                stack,
                EffectResult::default(),
                events,
            );
        }

        let candidates = cohort
            .iter()
            .map(|reference| reference.object_id)
            .collect::<Vec<_>>();
        let count = candidates.len() as u32;
        let prompt =
            "Order the cards entering your graveyard, oldest first; the last card is on top."
                .to_string();
        let candidate_card_ids = candidates
            .iter()
            .map(|object_id| {
                self.state
                    .objects
                    .get(object_id)
                    .map(|object| object.card_id.clone())
                    .unwrap_or_default()
            })
            .collect();
        let candidate_names = candidates
            .iter()
            .map(|&object_id| events::object_display_name(&self.state, self.registry, object_id))
            .collect();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: 0,
                    deciding_player_id: owner,
                    source_object_id: stack.item.id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::SimultaneousEntryOrder as i32,
                    candidate_object_ids: candidates.clone(),
                    candidate_card_ids,
                    candidate_names,
                    min: count,
                    max: count,
                    candidate_selectable: vec![true; candidates.len()],
                    ordered: true,
                    public_reveal: None,
                    ..Default::default()
                },
            )),
        });
        events.push(events::ev_log_private(prompt.clone(), owner));
        self.state.pending_resolution = Some(PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,
            deciding_player: owner,
            presentation: PendingResolutionPresentation {
                source_object_id: stack.item.id,
                candidates,
                min: count,
                max: count,
                ordered: true,
                prompt,
                choice_kind: rv1::ChoiceKind::SimultaneousEntryOrder,
                unique_names: false,
            },
            continuation: ResolutionContinuation::ManifestDreadGraveyardOrder {
                order: Box::new(PendingManifestDreadGraveyardOrder {
                    stack,
                    owner,
                    cohort,
                }),
            },
        });
        Ok(finish_with_events(self, events))
    }

    pub(in crate::engine) fn finish_manifest_dread_graveyard_order(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::ManifestDreadGraveyardOrder { order } = &pending.continuation
        else {
            return Err(EngineError::Illegal(
                "Manifest Dread graveyard-order continuation missing",
            ));
        };
        let candidates = order
            .cohort
            .iter()
            .map(|reference| reference.object_id)
            .collect::<Vec<_>>();
        let same_cohort = chosen.len() == 2
            && chosen.iter().copied().collect::<HashSet<_>>().len() == 2
            && chosen
                .iter()
                .all(|object_id| candidates.contains(object_id))
            && pending.presentation.candidates == candidates
            && pending.presentation.ordered
            && pending.presentation.min == 2
            && pending.presentation.max == 2
            && pending.deciding_player == order.owner;
        let cohort_current = self.manifest_dread_library_cohort_current(order.owner, &order.cohort)
            && order.cohort[0].library_position == 0
            && order.cohort[1].library_position == 1;
        if !same_cohort || !cohort_current {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "Manifest Dread graveyard-order cohort is stale or invalid",
            ));
        }

        let stack = order.stack.clone();
        let owner = order.owner;
        let mut events = Vec::new();
        let moves = chosen
            .iter()
            .map(|&object_id| (object_id, Zone::Graveyard, None))
            .collect::<Vec<_>>();
        let receipts = match self.commit_observed_zone_move_batch(&moves, &mut events) {
            Ok(receipts) => receipts,
            Err(error) => {
                self.state.pending_resolution = Some(pending);
                return Err(error);
            }
        };
        for receipt in receipts {
            let reference = order
                .cohort
                .iter()
                .find(|reference| reference.object_id == receipt.object_id)
                .expect("validated Manifest Dread cohort member");
            events.push(permanent_moved_event_with_library_position(
                &self.state,
                receipt.object_id,
                owner,
                rv1::permanent_moved::Destination::Graveyard,
                reference.library_position,
            ));
        }
        self.complete_parked_stack_with_previous(stack, EffectResult::default(), events)
    }

    pub(in crate::engine) fn refresh_manifest_dread_graveyard_order_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.clone() else {
            return Ok(());
        };
        let ResolutionContinuation::ManifestDreadGraveyardOrder { order } = &pending.continuation
        else {
            return Ok(());
        };
        let owner_live = self
            .state
            .player_idx(order.owner)
            .is_some_and(|idx| !self.state.players[idx].has_lost);
        if owner_live && self.manifest_dread_library_cohort_current(order.owner, &order.cohort) {
            return Ok(());
        }

        self.state.pending_resolution = None;
        let completed = self.complete_parked_stack_with_previous(
            order.stack.clone(),
            EffectResult::default(),
            Vec::new(),
        )?;
        events.extend(completed.events);
        Ok(())
    }
}

#[cfg(test)]
mod skipped_entry_tests {
    use super::*;
    use crate::state::{TriggerAbilityOrigin, TriggerUseKey};
    use tricerules_card_model::{AbilityDefinitionId, ProgramOwnerId};

    // Prepared-state coverage of the completion boundary, not a claim that the admitted
    // Equipment owners can currently reach prohibited manifest entry in a legal session.
    fn fixture(
        card_id: &str,
        count: usize,
    ) -> (
        GameEngine,
        ObjectId,
        Vec<ManifestDreadLibraryRef>,
        ParkedStackResolution,
    ) {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            272_090,
            &[10, 20],
            20,
            None,
            true,
        )
        .unwrap();
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        let source = engine.state.players[0].hand[0];
        engine.state.objects.get_mut(&source).unwrap().card_id = card_id.into();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            Some(10),
        )
        .unwrap();
        let face = engine.registry.get(card_id).unwrap().primary_face();
        let ability = face.triggered_abilities[0].clone();
        let definition = AbilityDefinitionId {
            card_id: card_id.into(),
            face_id: face.face_id.clone(),
            ability_path: vec![ability.ability_id.clone()],
        };
        let mut item = engine.observer_return_item(source, 10);
        item.id = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        item.is_copy = false;
        item.is_triggered = true;
        item.source_permanent_id = Some(source);
        item.source_owner = Some(10);
        item.source_zone_change = engine.state.zone_change_generation[&source];
        item.triggered_ability = Some(ability);
        item.trigger_use_key = Some(TriggerUseKey {
            object_id: source,
            zone_change_generation: item.source_zone_change,
            ability_origin: TriggerAbilityOrigin::Printed(definition.clone()),
        });
        let program = engine
            .begin_program_run(&item, vec![ProgramOwnerId::ability(definition)])
            .unwrap();
        engine
            .run_program_with_previous(&item, program, EffectResult::default(), &mut Vec::new())
            .unwrap();
        let pending = engine
            .state
            .pending_resolution
            .take()
            .expect("real Manifest Dread Program parked");
        let ResolutionContinuation::ManifestDread {
            stack,
            mut looked_at,
        } = pending.continuation
        else {
            panic!("Manifest Dread choice")
        };
        assert!(stack.program.as_ref().unwrap().in_flight.is_some());
        looked_at.truncate(count);
        if count == 1 {
            engine.state.players[0].library.truncate(1);
        }
        engine
            .state
            .objects
            .get_mut(&looked_at[0].object_id)
            .unwrap()
            .face_down = true;
        (engine, source, looked_at, stack)
    }

    #[test]
    fn skipped_entry_does_not_turn_a_later_incarnation_face_up() {
        for count in [1, 2] {
            let (mut engine, equipment, refs, stack) = fixture("conductive_machete", count);
            let skipped = refs[0].object_id;
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                skipped,
                Zone::Exile,
                None,
            )
            .unwrap();
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                skipped,
                Zone::Battlefield,
                Some(10),
            )
            .unwrap();
            engine.state.objects.get_mut(&skipped).unwrap().face_down = true;
            let generation = engine.state.zone_change_generation[&skipped];
            engine
                .finish_manifest_dread_skipped_entry(stack, skipped, 10, refs, Vec::new())
                .unwrap();
            assert!(
                engine.state.objects[&skipped].face_down,
                "a stale skipped entry must not modify a later incarnation"
            );
            assert_eq!(engine.state.objects[&skipped].zone, Zone::Battlefield);
            assert_eq!(engine.state.zone_change_generation[&skipped], generation);
            assert_eq!(engine.state.objects[&equipment].attached_to, None);
            assert!(engine.state.pending_resolution.is_none());
        }
    }

    fn assert_completed(engine: &GameEngine, equipment: ObjectId, batches: &[RuledEventBatch]) {
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(
            engine.state.objects[&equipment].attached_to, None,
            "the real AttachEquipment tail receives an empty ManifestedPermanent result"
        );
        assert!(!batches
            .iter()
            .flat_map(|batch| &batch.events)
            .any(|event| matches!(
            &event.ev, Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                if moved.destination == rv1::permanent_moved::Destination::Battlefield as i32)));
        assert_eq!(
            batches
                .iter()
                .flat_map(|batch| &batch.events)
                .filter(|event| matches!(&event.ev, Some(rv1::ruled_event::Ev::PriorityChanged(_))))
                .count(),
            1,
            "one terminal continuation resumes priority exactly once"
        );
    }

    fn submit(
        engine: &mut GameEngine,
        player: PlayerId,
        chosen: Vec<ObjectId>,
    ) -> Result<RuledEventBatch, EngineError> {
        engine.submit_resolution_choice(
            player,
            &rv1::SubmitResolutionChoice {
                chosen_object_ids: chosen,
                ..Default::default()
            },
        )
    }

    #[test]
    fn one_card_skipped_entry_restores_face_and_finishes_each_real_equipment_program() {
        for card_id in ["conductive_machete", "cursed_windbreaker", "killers_mask"] {
            let (mut engine, equipment, refs, stack) = fixture(card_id, 1);
            let card = refs[0].object_id;
            let generation = refs[0].zone_change_generation;
            let graveyard = engine.state.players[0].graveyard.clone();
            let batch = engine
                .finish_manifest_dread_skipped_entry(stack, card, 10, refs, Vec::new())
                .unwrap();
            assert!(!engine.state.objects[&card].face_down);
            assert_eq!(engine.state.objects[&card].zone, Zone::Graveyard);
            assert_eq!(engine.state.zone_change_generation[&card], generation + 1);
            assert_eq!(
                engine.state.players[0].graveyard,
                [graveyard, vec![card]].concat()
            );
            assert!(engine.state.players[0].library.is_empty());
            assert_completed(&engine, equipment, &[batch]);
            let before = serde_json::to_value(&engine.state).unwrap();
            assert!(
                submit(&mut engine, 10, vec![card]).is_err(),
                "completion cannot be answered again"
            );
            assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
        }
    }

    #[test]
    fn two_card_skipped_entry_validates_and_commits_each_graveyard_order_once() {
        for card_id in ["conductive_machete", "cursed_windbreaker", "killers_mask"] {
            for reverse in [false, true] {
                let (mut engine, equipment, refs, stack) = fixture(card_id, 2);
                let ids: Vec<_> = refs.iter().map(|reference| reference.object_id).collect();
                let library = engine.state.players[0].library.clone();
                let graveyard = engine.state.players[0].graveyard.clone();
                let batch = engine
                    .finish_manifest_dread_skipped_entry(
                        stack,
                        ids[0],
                        10,
                        refs.clone(),
                        Vec::new(),
                    )
                    .unwrap();
                assert!(!engine.state.objects[&ids[0]].face_down);
                assert_eq!(engine.state.players[0].library, library);
                assert_eq!(engine.state.players[0].graveyard, graveyard);
                let pending = engine.state.pending_resolution.as_ref().unwrap();
                assert_eq!(pending.deciding_player, 10);
                assert_eq!(
                    pending.presentation.choice_kind,
                    rv1::ChoiceKind::SimultaneousEntryOrder
                );
                assert!(pending.presentation.ordered);
                assert_eq!(pending.presentation.candidates, ids);
                assert_eq!((pending.presentation.min, pending.presentation.max), (2, 2));
                let ResolutionContinuation::ManifestDreadGraveyardOrder { order } =
                    &pending.continuation
                else {
                    panic!("order continuation")
                };
                assert_eq!(
                    order.stack.program.as_ref().unwrap().frames[0].next_instruction,
                    1,
                    "the real attach tail waits until graveyard ordering commits"
                );
                for (actor, answer) in [
                    (20, ids.clone()),
                    (10, vec![]),
                    (10, vec![ids[0]]),
                    (10, vec![ids[0], ids[0]]),
                    (10, vec![ids[0], equipment]),
                ] {
                    let before = serde_json::to_value(&engine.state).unwrap();
                    assert!(submit(&mut engine, actor, answer).is_err());
                    assert_eq!(
                        serde_json::to_value(&engine.state).unwrap(),
                        before,
                        "invalid answers preserve the exact pending Program and all zones"
                    );
                }
                let mut chosen = ids.clone();
                if reverse {
                    chosen.reverse();
                }
                let completed = submit(&mut engine, 10, chosen.clone()).unwrap();
                assert_eq!(
                    engine.state.players[0].graveyard,
                    [graveyard, chosen].concat()
                );
                assert_eq!(
                    engine.state.players[0].library,
                    library.into_iter().skip(2).collect::<VecDeque<_>>()
                );
                for reference in refs {
                    assert_eq!(
                        engine.state.objects[&reference.object_id].zone,
                        Zone::Graveyard
                    );
                    assert!(!engine.state.objects[&reference.object_id].face_down);
                    assert_eq!(
                        engine.state.zone_change_generation[&reference.object_id],
                        reference.zone_change_generation + 1
                    );
                }
                assert_completed(&engine, equipment, &[batch, completed]);
                let before = serde_json::to_value(&engine.state).unwrap();
                assert!(submit(&mut engine, 10, ids).is_err());
                assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
            }
        }
    }

    #[test]
    fn parked_graveyard_order_rejects_later_incarnation_without_rebinding() {
        let (mut engine, equipment, refs, stack) = fixture("conductive_machete", 2);
        let ids: Vec<_> = refs.iter().map(|reference| reference.object_id).collect();
        engine
            .finish_manifest_dread_skipped_entry(stack, ids[0], 10, refs, Vec::new())
            .unwrap();
        // Preserve the same ObjectId, owner and Library position; only its incarnation changed.
        *engine
            .state
            .zone_change_generation
            .entry(ids[1])
            .or_default() += 2;
        let before = serde_json::to_value(&engine.state).unwrap();
        assert!(submit(&mut engine, 10, ids.clone()).is_err());
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
        let mut events = Vec::new();
        engine
            .refresh_manifest_dread_graveyard_order_departure(&mut events)
            .unwrap();
        assert!(engine.state.players[0].graveyard.is_empty());
        assert!(ids
            .iter()
            .all(|id| engine.state.objects[id].zone == Zone::Library));
        assert_completed(
            &engine,
            equipment,
            &[RuledEventBatch {
                events,
                ..Default::default()
            }],
        );
        let before = serde_json::to_value(&engine.state).unwrap();
        let mut repeated = Vec::new();
        engine
            .refresh_manifest_dread_graveyard_order_departure(&mut repeated)
            .unwrap();
        assert!(repeated.is_empty());
        assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
    }

    #[test]
    fn skipped_second_card_completion_replays_deterministically() {
        let mut outcomes = Vec::new();
        for _ in 0..2 {
            let (mut engine, equipment, refs, stack) = fixture("killers_mask", 2);
            let ids: Vec<_> = refs.iter().map(|reference| reference.object_id).collect();
            engine.state.objects.get_mut(&ids[0]).unwrap().face_down = false;
            engine.state.objects.get_mut(&ids[1]).unwrap().face_down = true;
            let parked = engine
                .finish_manifest_dread_skipped_entry(stack, ids[1], 10, refs, Vec::new())
                .unwrap();
            assert!(!engine.state.objects[&ids[1]].face_down);
            let completed = submit(&mut engine, 10, vec![ids[1], ids[0]]).unwrap();
            assert_completed(&engine, equipment, &[parked.clone(), completed.clone()]);
            outcomes.push((
                serde_json::to_value(&engine.state).unwrap(),
                parked.encode_to_vec(),
                completed.encode_to_vec(),
            ));
        }
        assert_eq!(
            outcomes[0], outcomes[1],
            "identical seed and completion choices replay identically"
        );
    }
}
