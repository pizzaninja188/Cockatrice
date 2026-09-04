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
        stack: ParkedStackResolution,
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
                    object_id: oid,
                    deciding_player: controller,
                    destination_controller: controller,
                    battle_protector: None,
                    face_index: 0,
                    unlock_room_door: None,
                    chosen_x: 0,
                    cast_cost_receipts: Vec::new(),
                    player_life_snapshot: self.player_life_snapshot(),
                    tapped: progress.tapped,
                    entry_counters: BTreeMap::new(),
                    applied_effects: Vec::new(),
                },
                completion,
                &mut events,
            ) {
                super::replacement::BattlefieldEntryProgress::Parked => {
                    return Ok(finish_with_events(self, events));
                }
                super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                    self.commit_battlefield_entry(entry, None)?;
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
        }
        if progress.shuffle {
            crate::engine::shuffle_player_library_for_current_command(&mut self.state, controller);
            events.push(ev_log(format!("P{controller} shuffles their library.")));
        }
        if progress.searched_library {
            self.fire_triggers(&[GameEvent::LibrarySearched {
                searcher: controller,
                library_owner: controller,
            }]);
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
                slots: Vec::new(),
                zones: selected.expect("validated selected zones"),
                destination,
                conditional_destination,
                shuffle,
                reveal,
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
                slots,
                zones,
                destination,
                conditional_destination,
                shuffle,
                reveal,
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
            tricerules_cards::primitives::GraveyardDestination::Hand => {
                self.commit_observed_zone_move(oid, Zone::Hand, None)?;
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
            tricerules_cards::primitives::GraveyardDestination::Battlefield { tapped } => {
                match self.begin_battlefield_entry(
                    stack.item.clone(),
                    BattlefieldEntryEvent {
                        object_id: oid,
                        deciding_player: controller,
                        destination_controller: controller,
                        battle_protector: None,
                        face_index: 0,
                        unlock_room_door: None,
                        chosen_x: 0,
                        cast_cost_receipts: Vec::new(),
                        player_life_snapshot: self.player_life_snapshot(),
                        tapped,
                        entry_counters: BTreeMap::new(),
                        applied_effects: Vec::new(),
                    },
                    BattlefieldEntryCompletion::ResolutionEffect {
                        owner: controller,
                        spell_label,
                        object_label: card_label,
                    },
                    &mut events,
                ) {
                    super::replacement::BattlefieldEntryProgress::Parked => {
                        return Ok(finish_with_events(self, events));
                    }
                    super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                        self.commit_battlefield_entry(entry, None)?;
                    }
                }
                events.push(permanent_moved_event(
                    &self.state,
                    oid,
                    controller,
                    rv1::permanent_moved::Destination::Battlefield,
                ));
            }
            tricerules_cards::primitives::GraveyardDestination::Exile => {
                self.commit_observed_zone_move(oid, Zone::Exile, None)?;
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
            tricerules_cards::primitives::GraveyardDestination::LibraryTop
            | tricerules_cards::primitives::GraveyardDestination::LibraryBottom => {
                let top =
                    destination == tricerules_cards::primitives::GraveyardDestination::LibraryTop;
                self.commit_observed_zone_move(oid, Zone::Library, None)?;
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
            candidate_generations,
            selection_slot_candidates,
            mut destination,
            conditional_destination,
            shuffle,
            reveal,
        ) = match &pending.continuation {
            ResolutionContinuation::SearchLibrary {
                stack,
                searcher,
                zones,
                candidate_generations,
                selection_slot_candidates,
                destination,
                conditional_destination,
                shuffle,
                reveal,
            } => (
                stack.clone(),
                *searcher,
                zones.clone(),
                candidate_generations.clone(),
                selection_slot_candidates.clone(),
                *destination,
                conditional_destination.clone(),
                *shuffle,
                *reveal,
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
            || (!selection_slot_candidates.is_empty()
                && !selection_admits_distinct_slots(chosen, &selection_slot_candidates))
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "library-search choice became stale or violates its slot assignment",
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
                SearchDestination::Hand => {
                    for &oid in chosen {
                        let card_name = object_display_name(&self.state, self.registry, oid);
                        let owner = self.state.objects.get(&oid).map(|object| object.owner);
                        let origin = self.state.objects.get(&oid).map(|object| object.zone);
                        if origin != Some(Zone::Hand) {
                            self.commit_observed_zone_move(oid, Zone::Hand, None)?;
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
                        self.commit_observed_zone_move(oid, Zone::Library, None)?;
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
                    return self.continue_library_search_battlefield_entries(
                        stack,
                        LibrarySearchEntryProgress {
                            searcher,
                            remaining_object_ids: chosen.to_vec(),
                            tapped,
                            shuffle,
                            searched_library,
                        },
                        ev,
                    );
                }
            }
        }

        if searched_library {
            self.fire_triggers(&[GameEvent::LibrarySearched {
                searcher: controller,
                library_owner: controller,
            }]);
        }
        self.complete_parked_resolution(stack.item, stack.resume_effect_index, ev)
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
