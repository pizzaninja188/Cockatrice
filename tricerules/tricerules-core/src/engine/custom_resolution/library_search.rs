use super::*;

use crate::state::LibrarySearchEntryProgress;

fn selection_admits_distinct_slots(chosen: &[ObjectId], slots: &[Vec<ObjectId>]) -> bool {
    fn assign(
        chosen_index: usize,
        chosen: &[ObjectId],
        slots: &[Vec<ObjectId>],
        occupied: &mut [bool],
    ) -> bool {
        if chosen_index == chosen.len() {
            return true;
        }
        for (slot_index, candidates) in slots.iter().enumerate() {
            if !occupied[slot_index] && candidates.contains(&chosen[chosen_index]) {
                occupied[slot_index] = true;
                if assign(chosen_index + 1, chosen, slots, occupied) {
                    return true;
                }
                occupied[slot_index] = false;
            }
        }
        false
    }

    chosen.len() <= slots.len() && assign(0, chosen, slots, &mut vec![false; slots.len()])
}

impl GameEngine {
    pub(in crate::engine) fn continue_library_search_battlefield_entries(
        &mut self,
        mut stack: ParkedStackResolution,
        mut progress: LibrarySearchEntryProgress,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let controller = progress.searcher;
        while !progress.remaining_object_ids.is_empty() {
            let oid = progress.remaining_object_ids.remove(0);
            let object = self
                .state
                .objects
                .get(&oid)
                .ok_or(EngineError::Illegal("searched card is stale"))?;
            let owner = object.owner;
            let card_label = self
                .registry
                .get(&object.card_id)
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| "card".to_string());
            let completion = BattlefieldEntryCompletion::LibrarySearch {
                owner,
                card_label: card_label.clone(),
                progress: progress.clone(),
            };
            match self.begin_battlefield_entry(
                stack.item.clone(),
                BattlefieldEntryEvent {
                    entry_reveal_receipts: Vec::new(),
                    mana_colors_spent_to_cast: Default::default(),
                    prepared: false,
                    object_id: oid,
                    deciding_player: controller,
                    destination_controller: controller,
                    battle_protector: None,
                    face_index: 0,
                    unlock_room_door: None,
                    chosen_x: 0,
                    cast_by: None,
                    cast_cost_receipts: Vec::new(),
                    player_life_snapshot: self.player_life_snapshot(),
                    tapped: progress.tapped,
                    set_types: None,
                    chosen_basic_land_type: None,
                    chosen_opponents: Vec::new(),
                    entry_counters: BTreeMap::new(),
                    entry_modifiers: Vec::new(),
                    attached_to: None,
                    pending_copy_candidate: None,
                    pending_aura_recipient: None,
                    accepted_aura_recipient: None,
                    applied_effects: Vec::new(),
                },
                completion,
                &mut events,
            ) {
                super::replacement::BattlefieldEntryProgress::Parked => {
                    return Ok(finish_with_events(self, events));
                }
                super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                    let entry = *entry;
                    self.commit_battlefield_entry(entry, None, &mut events)?;
                }
                super::replacement::BattlefieldEntryProgress::Skipped(entry) => {
                    self.restore_skipped_battlefield_entry(&entry)?;
                    continue;
                }
            }
            events.push(ev_log(format!(
                "P{controller} puts {card_label} onto the battlefield."
            )));
            events.push(permanent_moved_event(
                &self.state,
                oid,
                owner,
                rv1::permanent_moved::Destination::Battlefield,
            ));
            if let Some(result_id) = progress.result_id.take() {
                stack.item.search_results.insert(
                    result_id,
                    TriggerObjectRef {
                        object_id: oid,
                        zone_change_generation: self
                            .state
                            .zone_change_generation
                            .get(&oid)
                            .copied()
                            .unwrap_or(0),
                        controller_at_event: controller,
                    },
                );
            }
        }
        for oid in progress.hand_object_ids {
            let card_label = self
                .state
                .objects
                .get(&oid)
                .and_then(|object| self.registry.get(&object.card_id))
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| "card".to_string());
            let owner = self
                .state
                .objects
                .get(&oid)
                .map(|object| object.owner)
                .ok_or(EngineError::Illegal("searched card is stale"))?;
            self.commit_observed_zone_move(oid, Zone::Hand, None, &mut events)?;
            events.push(permanent_moved_event(
                &self.state,
                oid,
                owner,
                rv1::permanent_moved::Destination::Hand,
            ));
            events.push(ev_log(format!(
                "P{controller} puts {card_label} into their hand."
            )));
        }
        if progress.shuffle {
            crate::engine::shuffle_player_library_for_current_command(&mut self.state, controller);
            events.push(ev_log(format!("P{controller} shuffles their library.")));
        }
        if progress.searched_library {
            self.fire_triggers(
                &[GameEvent::LibrarySearched {
                    searcher: controller,
                    library_owner: controller,
                }],
                &mut events,
            );
        }
        self.complete_parked_resolution(stack.item, stack.resume_effect_index, events)
    }

    pub(super) fn finish_search_zone_scope(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let (
            stack,
            searcher,
            count,
            available_zones,
            filter,
            destination,
            conditional_destination,
            shuffle,
            reveal,
        ) = match &pending.continuation {
            ResolutionContinuation::SearchZoneScope {
                stack,
                searcher,
                count,
                available_zones,
                filter,
                destination,
                conditional_destination,
                shuffle,
                reveal,
            } => (
                stack.clone(),
                *searcher,
                *count,
                available_zones.clone(),
                filter.clone(),
                *destination,
                conditional_destination.clone(),
                *shuffle,
                *reveal,
            ),
            _ => return Err(EngineError::Illegal("search-zone continuation missing")),
        };
        let combinations = resolution::zones::search_zone_combinations(&available_zones);
        let selected = combinations
            .get(answer.selected_branch_index as usize)
            .cloned();
        if decision != rv1::ResolutionChoiceDecision::SelectBranch
            || !answer.chosen_object_ids.is_empty()
            || selected.is_none()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("invalid search-zone selection"));
        }
        let mut events = Vec::new();
        resolution::zones::park_zone_search_choice(
            self,
            &mut events,
            &stack.item,
            searcher,
            resolution::zones::ZoneSearchRequest {
                count,
                filter,
                selection_constraint: None,
                slots: Vec::new(),
                zones: selected.expect("validated selected zones"),
                destination,
                conditional_destination,
                shuffle,
                reveal,
                result_id: None,
            },
        )?;
        let next = self
            .state
            .pending_resolution
            .as_mut()
            .and_then(|pending| pending.continuation.stack_mut())
            .ok_or(EngineError::Illegal("zone search failed to park"))?;
        next.resume_effect_index = stack.resume_effect_index;
        next.previous_result = stack.previous_result;
        Ok(finish_with_events(self, events))
    }

    pub(super) fn finish_optional_search_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let (
            stack,
            searcher,
            count,
            filter,
            slots,
            zones,
            destination,
            conditional_destination,
            shuffle,
            reveal,
        ) = match &pending.continuation {
            ResolutionContinuation::OptionalSearch {
                stack,
                searcher,
                count,
                filter,
                slots,
                zones,
                destination,
                conditional_destination,
                shuffle,
                reveal,
            } => (
                stack.clone(),
                *searcher,
                *count,
                filter.clone(),
                slots.clone(),
                zones.clone(),
                *destination,
                conditional_destination.clone(),
                *shuffle,
                *reveal,
            ),
            _ => return Err(EngineError::Illegal("optional-search continuation missing")),
        };
        let invalid = !answer.chosen_object_ids.is_empty()
            || match decision {
                rv1::ResolutionChoiceDecision::Decline => false,
                rv1::ResolutionChoiceDecision::SelectBranch => answer.selected_branch_index != 0,
                _ => true,
            };
        if invalid {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("invalid optional-search selection"));
        }
        let mut events = Vec::new();
        if decision == rv1::ResolutionChoiceDecision::Decline {
            events.push(ev_log(format!(
                "P{searcher} declines to search their library."
            )));
            return self.complete_parked_resolution_with_previous(
                stack.item,
                stack.resume_effect_index,
                stack.previous_result,
                events,
            );
        }
        resolution::zones::begin_search_request(
            self,
            &mut events,
            &stack.item,
            searcher,
            resolution::zones::SearchRequest {
                count,
                filter,
                selection_constraint: None,
                slots,
                zones,
                destination,
                conditional_destination,
                shuffle,
                reveal,
                result_id: None,
            },
        )?;
        let Some(next) = self
            .state
            .pending_resolution
            .as_mut()
            .and_then(|pending| pending.continuation.stack_mut())
        else {
            return Err(EngineError::Illegal("optional search failed to park"));
        };
        next.resume_effect_index = stack.resume_effect_index;
        next.previous_result = stack.previous_result;
        Ok(finish_with_events(self, events))
    }

    pub(super) fn finish_owner_library_placement(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, object_id, owner, generation, nonbottom_placement, spell_label) =
            match &pending.continuation {
                ResolutionContinuation::OwnerLibraryPlacement {
                    stack,
                    object_id,
                    owner,
                    zone_change_generation,
                    nonbottom_placement,
                    spell_label,
                } => (
                    stack.clone(),
                    *object_id,
                    *owner,
                    *zone_change_generation,
                    *nonbottom_placement,
                    spell_label.clone(),
                ),
                _ => return Err(EngineError::Illegal("owner-placement continuation missing")),
            };
        let invalid_shape = decision != rv1::ResolutionChoiceDecision::SelectBranch
            || !answer.chosen_object_ids.is_empty()
            || answer.selected_branch_index > 1;
        let current_generation = self
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0);
        let stale = !self
            .state
            .objects
            .get(&object_id)
            .is_some_and(|object| object.zone == Zone::Battlefield && object.owner == owner)
            || current_generation != generation;
        if invalid_shape || stale {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(if stale {
                "owner-placement target became stale"
            } else {
                "owner placement requires the authored non-bottom placement or Bottom"
            }));
        }

        let placement = if answer.selected_branch_index == 0 {
            nonbottom_placement
        } else {
            LibraryPlacement::Bottom
        };
        let mut events = Vec::new();
        resolution::zones::move_permanent_to_owners_library(
            self,
            &mut events,
            object_id,
            placement,
            &spell_label,
        )?;
        self.complete_parked_resolution(stack.item, stack.resume_effect_index, events)
    }

    pub(super) fn finish_graveyard_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, destination, generations, spell_label) = match &pending.continuation {
            ResolutionContinuation::GraveyardChoice {
                stack,
                destination,
                candidate_generations,
                spell_label,
            } => (
                stack.clone(),
                *destination,
                candidate_generations.clone(),
                spell_label.clone(),
            ),
            _ => {
                return Err(EngineError::Illegal(
                    "graveyard-choice continuation missing",
                ))
            }
        };
        let controller = stack.item.controller;
        let mut events = Vec::new();
        let Some(&oid) = chosen.first() else {
            events.push(ev_log(format!(
                "P{controller} declines to return a graveyard card."
            )));
            return self.complete_parked_resolution(stack.item, stack.resume_effect_index, events);
        };
        let expected_generation = generations
            .iter()
            .find_map(|(candidate, generation)| (*candidate == oid).then_some(*generation));
        let current_generation = self
            .state
            .zone_change_generation
            .get(&oid)
            .copied()
            .unwrap_or(0);
        if expected_generation != Some(current_generation)
            || !self
                .state
                .objects
                .get(&oid)
                .is_some_and(|object| object.zone == Zone::Graveyard && object.owner == controller)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("graveyard choice became stale"));
        }
        let card_label = object_display_name(&self.state, self.registry, oid);
        match destination {
            tricerules_card_model::primitives::GraveyardDestination::Hand => {
                self.commit_observed_zone_move(oid, Zone::Hand, None, &mut events)?;
                events.push(ev_log(format!(
                    "{spell_label} returns {card_label} from graveyard to hand."
                )));
                events.push(permanent_moved_event(
                    &self.state,
                    oid,
                    controller,
                    rv1::permanent_moved::Destination::Hand,
                ));
            }
            tricerules_card_model::primitives::GraveyardDestination::Battlefield { tapped } => {
                match self.begin_battlefield_entry(
                    stack.item.clone(),
                    BattlefieldEntryEvent {
                        entry_reveal_receipts: Vec::new(),
                        mana_colors_spent_to_cast: Default::default(),
                        prepared: false,
                        object_id: oid,
                        deciding_player: controller,
                        destination_controller: controller,
                        battle_protector: None,
                        face_index: 0,
                        unlock_room_door: None,
                        chosen_x: 0,
                        cast_by: None,
                        cast_cost_receipts: Vec::new(),
                        player_life_snapshot: self.player_life_snapshot(),
                        tapped,
                        set_types: None,
                        chosen_basic_land_type: None,
                        chosen_opponents: Vec::new(),
                        entry_counters: BTreeMap::new(),
                        entry_modifiers: Vec::new(),
                        attached_to: None,
                        pending_copy_candidate: None,
                        pending_aura_recipient: None,
                        accepted_aura_recipient: None,
                        applied_effects: Vec::new(),
                    },
                    BattlefieldEntryCompletion::ResolutionEffect {
                        owner: controller,
                        spell_label,
                        object_label: card_label,
                        from_zone: Zone::Graveyard,
                    },
                    &mut events,
                ) {
                    super::replacement::BattlefieldEntryProgress::Parked => {
                        return Ok(finish_with_events(self, events));
                    }
                    super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                        let entry = *entry;
                        self.commit_battlefield_entry(entry, None, &mut events)?;
                    }
                    super::replacement::BattlefieldEntryProgress::Skipped(entry) => {
                        self.restore_skipped_battlefield_entry(&entry)?;
                        return self.complete_parked_resolution_with_previous(
                            stack.item,
                            stack.resume_effect_index,
                            stack.previous_result,
                            events,
                        );
                    }
                }
                events.push(permanent_moved_event(
                    &self.state,
                    oid,
                    controller,
                    rv1::permanent_moved::Destination::Battlefield,
                ));
            }
            tricerules_card_model::primitives::GraveyardDestination::Exile => {
                self.commit_observed_zone_move(oid, Zone::Exile, None, &mut events)?;
                events.push(ev_log(format!(
                    "{spell_label} exiles {card_label} from the graveyard."
                )));
                events.push(permanent_moved_event(
                    &self.state,
                    oid,
                    controller,
                    rv1::permanent_moved::Destination::Exile,
                ));
            }
            tricerules_card_model::primitives::GraveyardDestination::LibraryTop
            | tricerules_card_model::primitives::GraveyardDestination::LibraryBottom => {
                let top = destination
                    == tricerules_card_model::primitives::GraveyardDestination::LibraryTop;
                self.commit_observed_zone_move(oid, Zone::Library, None, &mut events)?;
                if top {
                    let player_idx = self
                        .state
                        .player_idx(controller)
                        .ok_or(EngineError::Illegal("graveyard card owner not found"))?;
                    let library = &mut self.state.players[player_idx].library;
                    library.retain(|object_id| *object_id != oid);
                    library.push_front(oid);
                }
                let position = if top { "top" } else { "bottom" };
                events.push(ev_log(format!(
                    "{spell_label} puts {card_label} on the {position} of its owner's library."
                )));
                events.push(permanent_moved_event(
                    &self.state,
                    oid,
                    controller,
                    rv1::permanent_moved::Destination::Library,
                ));
            }
        }
        self.complete_parked_resolution(stack.item, stack.resume_effect_index, events)
    }

    /// CR 701.23: the controller submitted their library search choice. Move the found card to
    /// the declared destination, optionally reveal it publicly, then optionally shuffle.
    pub(super) fn finish_library_search(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let (
            stack,
            searcher,
            zones,
            filter,
            selection_constraint,
            candidate_generations,
            selection_slot_candidates,
            mut destination,
            conditional_destination,
            shuffle,
            reveal,
            result_id,
        ) = match &pending.continuation {
            ResolutionContinuation::SearchLibrary {
                stack,
                searcher,
                zones,
                filter,
                selection_constraint,
                candidate_generations,
                selection_slot_candidates,
                destination,
                conditional_destination,
                shuffle,
                reveal,
                result_id,
            } => (
                stack.clone(),
                *searcher,
                zones.clone(),
                filter.clone(),
                *selection_constraint,
                candidate_generations.clone(),
                selection_slot_candidates.clone(),
                *destination,
                conditional_destination.clone(),
                *shuffle,
                *reveal,
                result_id.clone(),
            ),
            _ => return Err(EngineError::Illegal("library-search continuation missing")),
        };
        let controller = searcher;
        let searched_library = zones.contains(&CardSearchZone::Library);
        let shuffle = shuffle && zones.contains(&CardSearchZone::Library);
        let choices_are_current = chosen.iter().all(|oid| {
            let expected_generation = candidate_generations
                .iter()
                .find_map(|(candidate, generation)| (*candidate == *oid).then_some(*generation));
            let current_generation = self
                .state
                .zone_change_generation
                .get(oid)
                .copied()
                .unwrap_or(0);
            expected_generation == Some(current_generation)
                && self.state.objects.get(oid).is_some_and(|object| {
                    object.owner == controller
                        && match object.zone {
                            Zone::Hand => zones.contains(&CardSearchZone::Hand),
                            Zone::Graveyard => zones.contains(&CardSearchZone::Graveyard),
                            Zone::Library => zones.contains(&CardSearchZone::Library),
                            _ => false,
                        }
                })
        });
        if !choices_are_current
            || !chosen.iter().all(|oid| {
                zone_card_matches_filter(&self.state, self.registry, *oid, filter.as_ref())
            })
            || !resolution::zones::search_selection_constraint_holds(
                self,
                chosen,
                selection_constraint,
            )
            || (!selection_slot_candidates.is_empty()
                && !selection_admits_distinct_slots(chosen, &selection_slot_candidates))
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "library-search choice became stale or violates its filter or selection constraint",
            ));
        }
        if let Some(conditional) = conditional_destination.filter(|conditional| {
            self.condition_holds(
                &conditional.condition,
                ConditionContext::for_stack_item(&stack.item),
            )
        }) {
            destination = conditional.destination;
        }

        let mut ev = vec![];

        if reveal {
            ev.extend(super::super::reveals::reveal_cards(
                &self.state,
                self.registry,
                chosen,
                stack.item.id,
                &object_display_name(&self.state, self.registry, stack.item.id),
            ));
        }

        if chosen.is_empty() {
            ev.push(ev_log(format!("P{controller} finds no card.")));
            if shuffle {
                crate::engine::shuffle_player_library_for_current_command(
                    &mut self.state,
                    controller,
                );
                ev.push(ev_log(format!("P{controller} shuffles their library.")));
            }
        } else {
            match destination {
                SearchDestination::Graveyard => {
                    for &oid in chosen {
                        let owner = self.state.objects[&oid].owner;
                        if self.state.objects[&oid].zone != Zone::Graveyard {
                            self.commit_observed_zone_move(oid, Zone::Graveyard, None, &mut ev)?;
                            ev.push(permanent_moved_event(
                                &self.state,
                                oid,
                                owner,
                                rv1::permanent_moved::Destination::Graveyard,
                            ));
                        }
                        let card_name = object_display_name(&self.state, self.registry, oid);
                        let destination_name = match self.state.objects[&oid].zone {
                            Zone::Graveyard => "their graveyard",
                            Zone::Exile => "exile",
                            _ => {
                                return Err(EngineError::Illegal(
                                    "searched card destination unavailable",
                                ))
                            }
                        };
                        ev.push(ev_log(format!(
                            "P{controller} puts {card_name} into {destination_name}."
                        )));
                    }
                    if shuffle {
                        crate::engine::shuffle_player_library_for_current_command(
                            &mut self.state,
                            controller,
                        );
                        ev.push(ev_log(format!("P{controller} shuffles their library.")));
                    }
                }
                SearchDestination::Hand => {
                    for &oid in chosen {
                        let card_name = object_display_name(&self.state, self.registry, oid);
                        let owner = self.state.objects.get(&oid).map(|object| object.owner);
                        let origin = self.state.objects.get(&oid).map(|object| object.zone);
                        if origin != Some(Zone::Hand) {
                            self.commit_observed_zone_move(oid, Zone::Hand, None, &mut ev)?;
                            if let Some(owner) = owner {
                                ev.push(permanent_moved_event(
                                    &self.state,
                                    oid,
                                    owner,
                                    rv1::permanent_moved::Destination::Hand,
                                ));
                            }
                        }
                        if reveal {
                            ev.push(ev_log(format!("P{controller} reveals {card_name}.")));
                            ev.push(ev_log(format!(
                                "P{controller} puts {card_name} into their hand."
                            )));
                        } else {
                            ev.push(ev_log_private(
                                format!("P{controller} puts {card_name} into their hand."),
                                controller,
                            ));
                            ev.push(ev_log_hidden_from(
                                format!("P{controller} puts a card into their hand."),
                                controller,
                            ));
                        }
                    }
                    if shuffle {
                        crate::engine::shuffle_player_library_for_current_command(
                            &mut self.state,
                            controller,
                        );
                        ev.push(ev_log(format!("P{controller} shuffles their library.")));
                    }
                }
                SearchDestination::TopOfLibrary => {
                    let oid = chosen[0];
                    let card_name = object_display_name(&self.state, self.registry, oid);
                    // Oracle: "then shuffle and put that card on top" — shuffle first, then put on top.
                    let owner = self.state.objects.get(&oid).map(|object| object.owner);
                    if self.state.objects.get(&oid).map(|object| object.zone) != Some(Zone::Library)
                    {
                        self.commit_observed_zone_move(oid, Zone::Library, None, &mut ev)?;
                        if let Some(owner) = owner {
                            ev.push(permanent_moved_event(
                                &self.state,
                                oid,
                                owner,
                                rv1::permanent_moved::Destination::Library,
                            ));
                        }
                    }
                    if let Some(idx) = self.state.player_idx(controller) {
                        self.state.players[idx].library.retain(|&x| x != oid);
                    }
                    if shuffle {
                        crate::engine::shuffle_player_library_for_current_command(
                            &mut self.state,
                            controller,
                        );
                        ev.push(ev_log(format!("P{controller} shuffles their library.")));
                    }
                    if let Some(idx) = self.state.player_idx(controller) {
                        self.state.players[idx].library.push_front(oid);
                    }
                    if let Some(o) = self.state.objects.get_mut(&oid) {
                        o.zone = Zone::Library;
                    }
                    if reveal {
                        ev.push(ev_log(format!("P{controller} reveals {card_name}.")));
                        ev.push(ev_log(format!(
                            "P{controller} puts {card_name} on top of their library."
                        )));
                    } else {
                        ev.push(ev_log_private(
                            format!("P{controller} puts {card_name} on top of their library."),
                            controller,
                        ));
                        ev.push(ev_log_hidden_from(
                            format!("P{controller} puts a card on top of their library."),
                            controller,
                        ));
                    }
                }
                SearchDestination::Battlefield { tapped } => {
                    if zones == [CardSearchZone::Library] {
                        let entries = chosen
                            .iter()
                            .map(|&oid| BattlefieldEntryEvent {
                                entry_reveal_receipts: Vec::new(),
                                mana_colors_spent_to_cast: Default::default(),
                                prepared: false,
                                object_id: oid,
                                deciding_player: controller,
                                destination_controller: controller,
                                battle_protector: None,
                                face_index: 0,
                                unlock_room_door: None,
                                chosen_x: 0,
                                cast_by: None,
                                cast_cost_receipts: Vec::new(),
                                player_life_snapshot: self.player_life_snapshot(),
                                tapped,
                                set_types: None,
                                chosen_basic_land_type: None,
                                chosen_opponents: Vec::new(),
                                entry_counters: BTreeMap::new(),
                                entry_modifiers: Vec::new(),
                                attached_to: None,
                                pending_copy_candidate: None,
                                pending_aura_recipient: None,
                                accepted_aura_recipient: None,
                                applied_effects: Vec::new(),
                            })
                            .collect();
                        let label = object_display_name(&self.state, self.registry, stack.item.id);
                        let Some(stack) = self.begin_zone_entry_batch(
                            stack,
                            entries,
                            Zone::Library,
                            &label,
                            Some(crate::state::ZoneEntryCompletion::LibrarySearch(
                                crate::state::LibrarySearchCompletion {
                                    searcher,
                                    shuffle,
                                    searched_library,
                                    result_id,
                                },
                            )),
                            &mut ev,
                        )?
                        else {
                            return Ok(finish_with_events(self, ev));
                        };
                        return self.complete_parked_resolution_with_previous(
                            stack.item,
                            stack.resume_effect_index,
                            stack.previous_result,
                            ev,
                        );
                    }
                    return self.continue_library_search_battlefield_entries(
                        stack,
                        LibrarySearchEntryProgress {
                            searcher,
                            remaining_object_ids: chosen.to_vec(),
                            hand_object_ids: Vec::new(),
                            tapped,
                            shuffle,
                            searched_library,
                            result_id,
                        },
                        ev,
                    );
                }
                SearchDestination::BattlefieldTappedThenHand => {
                    return self.continue_library_search_battlefield_entries(
                        stack,
                        LibrarySearchEntryProgress {
                            searcher,
                            remaining_object_ids: vec![chosen[0]],
                            hand_object_ids: chosen.iter().skip(1).copied().collect(),
                            tapped: true,
                            shuffle,
                            searched_library,
                            result_id,
                        },
                        ev,
                    );
                }
            }
        }

        if searched_library {
            self.fire_triggers(
                &[GameEvent::LibrarySearched {
                    searcher: controller,
                    library_owner: controller,
                }],
                &mut ev,
            );
        }
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            stack.previous_result,
            ev,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::replacement::PendingReplacementEvent;

    fn library_card(engine: &mut GameEngine, card_id: &str) -> ObjectId {
        let player = &mut engine.state.players[0];
        let object_id = player.hand.pop().expect("fixture card in hand");
        let object = engine
            .state
            .objects
            .get_mut(&object_id)
            .expect("fixture object");
        object.card_id = card_id.to_string();
        object.zone = Zone::Library;
        player.library.push_back(object_id);
        object_id
    }

    fn battlefield_card(engine: &mut GameEngine, card_id: &str) -> ObjectId {
        let player = &mut engine.state.players[0];
        let object_id = player.hand.pop().expect("fixture card in hand");
        let object = engine
            .state
            .objects
            .get_mut(&object_id)
            .expect("fixture object");
        object.card_id = card_id.to_string();
        object.zone = Zone::Battlefield;
        player.battlefield.push(object_id);
        object_id
    }

    fn test_stack_item() -> StackItem {
        StackItem {
            mana_colors_spent_to_cast: Default::default(),
            id: 90_001,
            controller: 0,
            card_id: "cultivate".into(),
            targets: Vec::new(),
            ability_text: None,
            source_permanent_id: None,
            source_owner: Some(0),
            source_zone_change: 0,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            returned_attacker_assignment: None,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            cast_cost_receipts: Vec::new(),
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: Some(0),
            payment_result: Default::default(),
            search_results: Default::default(),
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: Default::default(),
        }
    }

    #[test]
    fn graveyard_search_preserves_duplicate_identity_tail_and_one_search_completion() {
        // Deserialization keeps the intended red executable before the destination exists.
        let destination: SearchDestination = serde_json::from_str("\"Graveyard\"")
            .expect("library searches must support a graveyard destination");
        for find in [true, false] {
            let mut engine = GameEngine::new(
                tricerules_cards::registry::global(),
                90_030,
                &[0, 1, 2],
                20,
                None,
                true,
            )
            .unwrap();
            engine.state.turn_step = TurnStep::Main1;
            let wan = battlefield_card(&mut engine, "wan_shi_tong,_librarian");
            engine.state.players[0].battlefield.retain(|id| *id != wan);
            engine.state.players[1].battlefield.push(wan);
            let observer = engine.state.objects.get_mut(&wan).unwrap();
            observer.owner = 1;
            observer.controller = 1;
            observer.base_controller = 1;
            let unselected = library_card(&mut engine, "sol_ring");
            let selected = library_card(&mut engine, "sol_ring");
            let invalid = library_card(&mut engine, "lightning_bolt");
            let generation = engine
                .state
                .zone_change_generation
                .get(&selected)
                .copied()
                .unwrap_or(0);
            let mut item = test_stack_item();
            item.card_id = "myriad_landscape".into();
            item.ability_text = Some("graveyard search plus sentinel".into());
            let mut ability = engine
                .registry
                .get("myriad_landscape")
                .unwrap()
                .primary_face()
                .activated_abilities[1]
                .clone();
            // Search instruction is already parked; the saved tail must run once on completion.
            ability.effect.push(SpellEffectKind::GainLife {
                amount: Amount::Fixed(3),
            });
            item.activated_ability = Some(ability);
            let mut events = Vec::new();
            resolution::zones::park_zone_search_choice(
                &mut engine,
                &mut events,
                &item,
                0,
                resolution::zones::ZoneSearchRequest {
                    count: 1,
                    filter: Some(ZoneCardFilter {
                        card_type: Some(CardTypeFilter::Artifact),
                        ..Default::default()
                    }),
                    selection_constraint: None,
                    slots: Vec::new(),
                    zones: vec![CardSearchZone::Library],
                    destination,
                    conditional_destination: None,
                    shuffle: true,
                    reveal: false,
                    result_id: None,
                },
            )
            .unwrap();
            engine
                .state
                .pending_resolution
                .as_mut()
                .unwrap()
                .continuation
                .stack_mut()
                .unwrap()
                .resume_effect_index = Some(1);
            let candidates = engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates
                .clone();
            assert!(candidates.contains(&selected) && candidates.contains(&unselected));
            assert!(!candidates.contains(&invalid));
            assert!(
                engine
                    .submit_resolution_choice(
                        1,
                        &rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![selected],
                            ..Default::default()
                        }
                    )
                    .is_err(),
                "opponent cannot answer the private search"
            );
            assert!(
                engine
                    .submit_resolution_choice(
                        0,
                        &rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![invalid],
                            ..Default::default()
                        }
                    )
                    .is_err(),
                "nonartifact is not a candidate"
            );
            assert_eq!(engine.state.objects[&selected].zone, Zone::Library);
            engine
                .state
                .zone_change_generation
                .insert(selected, generation + 1);
            let before = format!("{:?}", engine.state);
            assert!(
                engine
                    .submit_resolution_choice(
                        0,
                        &rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![selected],
                            ..Default::default()
                        }
                    )
                    .is_err(),
                "a candidate receipt cannot select a later incarnation"
            );
            assert_eq!(format!("{:?}", engine.state), before);
            engine
                .state
                .zone_change_generation
                .insert(selected, generation);
            assert_eq!(engine.state.players[0].life, 20);
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .presentation
                    .candidates,
                candidates
            );
            let batch = engine
                .submit_resolution_choice(
                    0,
                    &rv1::SubmitResolutionChoice {
                        chosen_object_ids: if find { vec![selected] } else { Vec::new() },
                        ..Default::default()
                    },
                )
                .unwrap();
            assert!(engine.state.pending_resolution.is_none());
            assert_eq!(engine.state.players[0].life, 23);
            assert_eq!(engine.state.objects[&unselected].zone, Zone::Library);
            assert_eq!(engine.state.objects[&invalid].zone, Zone::Library);
            assert_eq!(
                engine.state.objects[&selected].zone,
                if find { Zone::Graveyard } else { Zone::Library }
            );
            assert_eq!(
                engine
                    .state
                    .zone_change_generation
                    .get(&selected)
                    .copied()
                    .unwrap_or(0),
                generation + u64::from(find)
            );
            let moves: Vec<_> = batch
                .events
                .iter()
                .filter_map(|event| match &event.ev {
                    Some(rv1::ruled_event::Ev::PermanentMoved(moved)) => Some(moved),
                    _ => None,
                })
                .collect();
            assert_eq!(moves.len(), usize::from(find));
            if find {
                assert_eq!(moves[0].object_id, selected);
                assert_eq!(
                    moves[0].destination(),
                    rv1::permanent_moved::Destination::Graveyard
                );
                assert_eq!(moves[0].card_id, "sol_ring");
                assert!(engine.state.players[0].graveyard.contains(&selected));
                assert!(!engine.state.players[0].library.contains(&selected));
            }
            assert_eq!(
                batch
                    .events
                    .iter()
                    .filter(|event| matches!(&event.ev,
                Some(rv1::ruled_event::Ev::Log(log)) if log.text == "P0 shuffles their library."))
                    .count(),
                1
            );
            engine.flush_staged_triggers(&mut Vec::new());
            assert_eq!(
                engine.state.stack.len(),
                1,
                "one search observer even on qualified fail-to-find"
            );
            assert_eq!(engine.state.stack[0].source_permanent_id, Some(wan));
        }
    }

    #[test]
    fn library_entry_cohort_preserves_previous_result_tail_and_one_search_completion() {
        for count in [2, 0] {
            let mut engine = GameEngine::new(
                tricerules_cards::registry::global(),
                90_020 + count,
                &[0, 1],
                20,
                None,
                true,
            )
            .unwrap();
            engine.state.turn_step = TurnStep::Main1;
            battlefield_card(&mut engine, "orb_of_dreams");
            battlefield_card(&mut engine, "orb_of_dreams");
            let wan = battlefield_card(&mut engine, "wan_shi_tong,_librarian");
            engine.state.players[0].battlefield.retain(|id| *id != wan);
            engine.state.players[1].battlefield.push(wan);
            let observer = engine.state.objects.get_mut(&wan).unwrap();
            observer.owner = 1;
            observer.controller = 1;
            observer.base_controller = 1;
            let forest = library_card(&mut engine, "forest");
            let second = library_card(&mut engine, "forest");
            let mut effect = engine
                .registry
                .get("grow_from_the_ashes")
                .unwrap()
                .primary_face()
                .spell_effect[0]
                .clone();
            let SpellEffectKind::SearchLibrary {
                count: search_count,
                count_by_cast_cost,
                reveal,
                ..
            } = &mut effect
            else {
                unreachable!()
            };
            *search_count = 2;
            *count_by_cast_cost = None;
            *reveal = false;
            let mut tail = engine
                .registry
                .get("fanatic_of_the_harrowing")
                .unwrap()
                .primary_face()
                .triggered_abilities[0]
                .effect[1]
                .clone();
            let SpellEffectKind::ChooseResolutionBranch { branches, .. } = &mut tail else {
                unreachable!()
            };
            let tricerules_card_model::primitives::ResolutionBranchRequirement::CardResultCount {
                filter,
                ..
            } = &mut branches[0].requirement
            else {
                unreachable!()
            };
            filter.action = tricerules_card_model::primitives::CardResultAction::Mill;
            branches[0].effects = vec![SpellEffectKind::GainLife {
                amount: Amount::Fixed(3),
            }];
            let mut item = test_stack_item();
            item.card_id = "myriad_landscape".into();
            item.ability_text = Some("search plus sentinel".into());
            let mut ability = engine
                .registry
                .get("myriad_landscape")
                .unwrap()
                .primary_face()
                .activated_abilities[1]
                .clone();
            ability.effect = vec![effect, tail];
            item.activated_ability = Some(ability);
            let mut events = Vec::new();
            resolution::zones::park_zone_search_choice(
                &mut engine,
                &mut events,
                &item,
                0,
                resolution::zones::ZoneSearchRequest {
                    count: 2,
                    filter: Some(ZoneCardFilter {
                        card_type: Some(CardTypeFilter::BasicLand),
                        ..Default::default()
                    }),
                    selection_constraint: None,
                    slots: Vec::new(),
                    zones: vec![CardSearchZone::Library],
                    destination: SearchDestination::Battlefield { tapped: false },
                    conditional_destination: None,
                    shuffle: true,
                    reveal: false,
                    result_id: None,
                },
            )
            .unwrap();
            let saved = engine
                .state
                .pending_resolution
                .as_mut()
                .unwrap()
                .continuation
                .stack_mut()
                .unwrap();
            saved.resume_effect_index = Some(1);
            saved
                .previous_result
                .cards
                .push(crate::state::CardResultEntry {
                    action: tricerules_card_model::primitives::CardResultAction::Mill,
                    affected_player: 0,
                    object_id: forest,
                    zone_change_generation: 0,
                    matched_card_types: vec![CardTypeFilter::BasicLand],
                });
            let selected = if count == 0 {
                Vec::new()
            } else {
                vec![forest, second]
            };
            let mut batches = vec![engine
                .submit_resolution_choice(
                    0,
                    &rv1::SubmitResolutionChoice {
                        chosen_object_ids: selected,
                        ..Default::default()
                    },
                )
                .unwrap()];
            for _ in 0..4 {
                let Some(pending) = engine.state.pending_resolution.as_ref() else {
                    break;
                };
                assert_eq!(
                    engine.state.players[0].life, 20,
                    "tail waits for the entire entry cohort"
                );
                assert_eq!(engine.state.objects[&forest].zone, Zone::Library);
                assert_eq!(engine.state.objects[&second].zone, Zone::Library);
                assert!(
                    engine.state.stack.is_empty(),
                    "LibrarySearched observer is not dispatched early"
                );
                let chosen = match pending.presentation.choice_kind {
                    custom::ChoiceKind::ReplacementEffect => {
                        vec![pending.presentation.candidates[0]]
                    }
                    custom::ChoiceKind::SimultaneousEntryOrder => pending
                        .presentation
                        .candidates
                        .iter()
                        .rev()
                        .copied()
                        .collect(),
                    other => panic!("unexpected entry choice {other:?}"),
                };
                batches.push(
                    engine
                        .submit_resolution_choice(
                            0,
                            &rv1::SubmitResolutionChoice {
                                chosen_object_ids: chosen,
                                ..Default::default()
                            },
                        )
                        .unwrap(),
                );
            }
            assert!(engine.state.pending_resolution.is_none());
            assert_eq!(
                engine.state.players[0].life, 23,
                "saved previous result admits the sentinel tail exactly once, count {count}"
            );
            assert_eq!(batches.iter().flat_map(|batch| &batch.events).filter(|event| matches!(&event.ev, Some(rv1::ruled_event::Ev::Log(log)) if log.text == "P0 shuffles their library.")).count(), 1);
            engine.flush_staged_triggers(&mut Vec::new());
            assert_eq!(
                engine.state.stack.len(),
                1,
                "one LibrarySearched observer after the tail"
            );
            assert_eq!(engine.state.stack[0].source_permanent_id, Some(wan));
        }
    }

    #[test]
    fn parked_library_search_entry_preserves_later_hand_destination() {
        // Cultivate's first card is already marked tapped when entry replacement checks begin,
        // so use a real two-replacement land-entry choice to exercise the same parked completion
        // payload. Multiversal Passage and Orb of Dreams both apply before the land enters.
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            90_001,
            &[0, 1],
            20,
            None,
            true,
        )
        .expect("engine");
        let orb = battlefield_card(&mut engine, "orb_of_dreams");
        let passage = library_card(&mut engine, "multiversal_passage");
        let to_hand = library_card(&mut engine, "forest");
        let progress = LibrarySearchEntryProgress {
            searcher: 0,
            remaining_object_ids: vec![passage],
            hand_object_ids: vec![to_hand],
            tapped: false,
            shuffle: true,
            searched_library: true,
            result_id: None,
        };

        let batch = engine
            .continue_library_search_battlefield_entries(
                ParkedStackResolution::new(test_stack_item()),
                progress,
                Vec::new(),
            )
            .expect("entry replacements park the search");
        let choice = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .expect("replacement order choice");
        assert_eq!(
            choice.choice_kind,
            rv1::ChoiceKind::ReplacementEffect as i32
        );
        assert_eq!(choice.replacement_options.len(), 2);
        assert_eq!(engine.state.objects[&passage].zone, Zone::Library);
        assert_eq!(engine.state.objects[&to_hand].zone, Zone::Library);
        match engine.state.pending_replacement_event.as_ref() {
            Some(PendingReplacementEvent::BattlefieldEntry(entry)) => match &entry.completion {
                BattlefieldEntryCompletion::LibrarySearch { progress, .. } => {
                    assert_eq!(progress.hand_object_ids, [to_hand]);
                }
                other => panic!("unexpected entry completion: {other:?}"),
            },
            other => panic!("expected parked battlefield entry: {other:?}"),
        }

        let orb_option = choice
            .replacement_options
            .iter()
            .position(|option| option.source_object_id == orb)
            .expect("Orb of Dreams replacement option");
        let type_choice = engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![choice.candidate_object_ids[orb_option]],
                    ..Default::default()
                },
            )
            .expect("apply Orb of Dreams first");
        assert_eq!(engine.state.objects[&to_hand].zone, Zone::Library);
        assert!(type_choice.events.iter().any(|event| {
            matches!(&event.ev, Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                if choice.choice_kind == rv1::ChoiceKind::ResolutionBranch as i32)
        }));

        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                },
            )
            .expect("choose Multiversal Passage's basic land type");
        assert_eq!(engine.state.objects[&to_hand].zone, Zone::Library);
        let completed = engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                },
            )
            .expect("decline the optional life payment");

        assert_eq!(engine.state.objects[&passage].zone, Zone::Battlefield);
        assert!(engine.state.objects[&passage].tapped);
        assert_eq!(engine.state.objects[&to_hand].zone, Zone::Hand);
        assert!(engine.state.players[0].hand.contains(&to_hand));
        assert_eq!(
            completed
                .events
                .iter()
                .filter(|event| matches!(&event.ev, Some(rv1::ruled_event::Ev::Log(log)) if log.text == "P0 shuffles their library."))
                .count(),
            1
        );
    }
}

#[cfg(test)]
mod selection_slot_tests {
    use super::selection_admits_distinct_slots;

    #[test]
    fn overlapping_candidates_use_a_distinct_assignment() {
        let slots = vec![vec![1, 2], vec![1]];
        assert!(selection_admits_distinct_slots(&[1, 2], &slots));
        assert!(selection_admits_distinct_slots(&[1], &slots));
        assert!(!selection_admits_distinct_slots(&[2, 3], &slots));
    }
}
