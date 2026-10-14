use super::*;

use crate::state::{LibrarySearchEntryProgress, PendingLinkedExileAction, SpecialCastCompletion};

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
    fn parked_choose_card_stack_is_current(&self, parked: &ParkedStackResolution) -> bool {
        let parked_item = &parked.item;
        match parked_item.cast_occurrence {
            Some(reference) if reference.object_id == parked_item.id => {
                match reference.zone_change_generation {
                    Some(generation) => {
                        self.state
                            .zone_change_generation
                            .get(&parked_item.id)
                            .copied()
                            == Some(generation)
                            && self
                                .state
                                .objects
                                .get(&parked_item.id)
                                .is_some_and(|object| {
                                    object.zone == Zone::Stack
                                        && !object.face_down
                                        && object.card_id == parked_item.card_id
                                        && object.face_up_index == parked_item.face_index
                                        && object.prototyped == parked_item.prototyped
                                })
                    }
                    None => true,
                }
            }
            Some(_) => false,
            None => true,
        }
    }

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
                },
                completion,
                &mut events,
            )? {
                super::replacement::BattlefieldEntryProgress::Parked => {
                    self.transfer_entry_choice_resume(&stack);
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

    pub(super) fn finish_choose_card(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, chooser, destination, candidates, spell_label) = match &pending.continuation {
            ResolutionContinuation::ChooseCard {
                stack,
                chooser,
                destination,
                candidates,
                spell_label,
            } => (
                stack.clone(),
                *chooser,
                *destination,
                candidates.clone(),
                spell_label.clone(),
            ),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("ChooseCard continuation missing"));
            }
        };
        if pending.deciding_player != chooser {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("ChooseCard decider changed"));
        }
        if !self.parked_choose_card_stack_is_current(&stack) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "ChooseCard stack resolution became stale",
            ));
        }
        let mut events = Vec::new();
        let Some(&oid) = chosen.first() else {
            events.push(ev_log(format!("P{chooser} declines to choose a card.")));
            let previous = stack.previous_result.clone();
            return self.complete_parked_stack_with_previous(stack, previous, events);
        };
        let Some(candidate) = candidates
            .iter()
            .find(|candidate| candidate.object_id == oid)
            .copied()
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("invalid ChooseCard candidate"));
        };
        let current_object = self.state.objects.get(&oid);
        if !super::super::resolution::zones::live_choose_card_candidate(
            self,
            oid,
            candidate.zone_change_generation,
            candidate.source_zone,
        ) || current_object.is_none()
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("ChooseCard candidate became stale"));
        }
        let owner = current_object.expect("candidate object checked").owner;
        let commander_owner = self.state.player_idx(owner).filter(|index| {
            !self.state.players[*index].has_lost
                && self.state.players[*index]
                    .declared_commander_object_ids
                    .contains(&oid)
        });
        if commander_owner.is_some()
            && matches!(
                destination,
                tricerules_card_model::primitives::GraveyardDestination::Hand
                    | tricerules_card_model::primitives::GraveyardDestination::LibraryTop
                    | tricerules_card_model::primitives::GraveyardDestination::LibraryBottom
            )
        {
            let destination_label = match destination {
                tricerules_card_model::primitives::GraveyardDestination::Hand => "its owner's hand",
                tricerules_card_model::primitives::GraveyardDestination::LibraryTop => {
                    "the top of its owner's library"
                }
                tricerules_card_model::primitives::GraveyardDestination::LibraryBottom => {
                    "the bottom of its owner's library"
                }
                _ => unreachable!("ChooseCard compiler limits ordinary movement destinations"),
            };
            let prompt =
                format!("P{owner}: put this commander in the command zone or {destination_label}?");
            let branches = ["Command zone", destination_label]
                .into_iter()
                .enumerate()
                .map(|(index, label)| rv1::ResolutionBranchOption {
                    branch_index: index as u32,
                    label: label.into(),
                    selectable: true,
                    ..Default::default()
                })
                .collect();
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                    rv1::ResolutionChoiceRequired {
                        resolution_prompt_id: 0,
                        deciding_player_id: owner,
                        source_object_id: stack.item.id,
                        prompt_text: prompt.clone(),
                        choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                        min: 1,
                        max: 1,
                        resolution_branches: branches,
                        ..Default::default()
                    },
                )),
            });
            events.push(ev_log(prompt.clone()));
            self.state.pending_resolution = Some(PendingResolution {
                resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
                resolution_prompt_semantics: None,
                deciding_player: owner,
                presentation: PendingResolutionPresentation {
                    source_object_id: stack.item.id,
                    candidates: Vec::new(),
                    min: 1,
                    max: 1,
                    ordered: false,
                    prompt,
                    choice_kind: custom::ChoiceKind::ResolutionBranch,
                    unique_names: false,
                },
                continuation: ResolutionContinuation::ChooseCardCommander {
                    stack,
                    chooser,
                    commander_owner: owner,
                    destination,
                    candidate,
                    spell_label,
                },
            });
            return Ok(finish_with_events(self, events));
        }
        self.commit_choose_card_move(stack, chooser, candidate, destination, false, &spell_label)
    }

    pub(super) fn finish_choose_card_commander(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::ChooseCardCommander {
            stack,
            chooser,
            commander_owner,
            destination,
            candidate,
            spell_label,
        } = pending.continuation.clone()
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "ChooseCard commander continuation missing",
            ));
        };
        let correct_shape = decision == rv1::ResolutionChoiceDecision::SelectBranch
            && answer.selected_branch_index <= 1
            && answer.chosen_object_ids.is_empty()
            && answer.chosen_player_ids.is_empty()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty()
            && answer.cast_spell.is_none()
            && answer.spell_cast_announcement.is_none()
            && answer.chosen_combat_defender.is_none();
        let owner_live = self.state.player_idx(commander_owner).is_some_and(|index| {
            !self.state.players[index].has_lost
                && self.state.players[index]
                    .declared_commander_object_ids
                    .contains(&candidate.object_id)
        });
        let target_live = self
            .state
            .objects
            .get(&candidate.object_id)
            .is_some_and(|object| object.owner == commander_owner)
            && super::super::resolution::zones::live_choose_card_candidate(
                self,
                candidate.object_id,
                candidate.zone_change_generation,
                candidate.source_zone,
            );
        let stack_live = self.parked_choose_card_stack_is_current(&stack);
        if !correct_shape
            || pending.deciding_player != commander_owner
            || !owner_live
            || !target_live
            || !stack_live
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "invalid or stale ChooseCard commander choice",
            ));
        }
        self.commit_choose_card_move(
            stack,
            chooser,
            candidate,
            destination,
            answer.selected_branch_index == 0,
            &spell_label,
        )
    }

    fn commit_choose_card_move(
        &mut self,
        stack: ParkedStackResolution,
        chooser: PlayerId,
        candidate: crate::state::CardChoiceCandidateRef,
        destination: tricerules_card_model::primitives::GraveyardDestination,
        command_zone: bool,
        spell_label: &str,
    ) -> Result<RuledEventBatch, EngineError> {
        let oid = candidate.object_id;
        if !self.parked_choose_card_stack_is_current(&stack) {
            return Err(EngineError::ProgramInvariant(
                "ChooseCard parked stack changed after answer validation",
            ));
        }
        if !super::super::resolution::zones::live_choose_card_candidate(
            self,
            oid,
            candidate.zone_change_generation,
            candidate.source_zone,
        ) {
            return Err(EngineError::ProgramInvariant(
                "ChooseCard candidate changed after answer validation",
            ));
        }
        let owner = self
            .state
            .objects
            .get(&oid)
            .map(|object| object.owner)
            .ok_or(EngineError::ProgramInvariant(
                "ChooseCard candidate disappeared before movement",
            ))?;
        let card_label = object_display_name(&self.state, self.registry, oid);
        if candidate.source_zone == Zone::Stack {
            let occurrence = Some(crate::state::StackObjectRef {
                object_id: oid,
                zone_change_generation: Some(candidate.zone_change_generation),
            });
            let index = self.state.stack.iter().position(|item| {
                item.id == oid
                    && item.ability_text.is_none()
                    && !item.is_copy
                    && item.cast_occurrence == occurrence
            });
            let Some(index) = index else {
                return Err(EngineError::ProgramInvariant(
                    "chosen Stack card lost its exact physical cast",
                ));
            };
            self.state.stack.remove(index);
            self.state.stack_presentations.remove(&oid);
            if self
                .state
                .millennium_calendar_state_trigger_stack_keys
                .remove(&oid)
                .is_some()
            {
                self.check_millennium_calendar_state_triggers();
            }
        }
        let mut events = Vec::new();
        if command_zone {
            self.commit_observed_zone_move(oid, Zone::Command, None, &mut events)?;
            events.push(permanent_moved_event(
                &self.state,
                oid,
                owner,
                rv1::permanent_moved::Destination::Command,
            ));
            events.push(ev_log(format!(
                "P{owner} puts {card_label} into the command zone instead."
            )));
        } else {
            match destination {
                tricerules_card_model::primitives::GraveyardDestination::Hand => {
                    self.commit_observed_zone_move(oid, Zone::Hand, None, &mut events)?;
                    events.push(ev_log(format!(
                        "{spell_label} puts {card_label} into P{owner}'s hand."
                    )));
                    events.push(permanent_moved_event(
                        &self.state,
                        oid,
                        owner,
                        rv1::permanent_moved::Destination::Hand,
                    ));
                }
                tricerules_card_model::primitives::GraveyardDestination::LibraryTop
                | tricerules_card_model::primitives::GraveyardDestination::LibraryBottom => {
                    let top = destination
                        == tricerules_card_model::primitives::GraveyardDestination::LibraryTop;
                    self.commit_observed_zone_move(oid, Zone::Library, None, &mut events)?;
                    let index = self
                        .state
                        .player_idx(owner)
                        .ok_or(EngineError::UnknownPlayer(owner))?;
                    let library = &mut self.state.players[index].library;
                    library.retain(|object_id| *object_id != oid);
                    if top {
                        library.push_front(oid);
                    } else {
                        library.push_back(oid);
                    }
                    let position = if top { "top" } else { "bottom" };
                    events.push(ev_log(format!(
                        "{spell_label} puts {card_label} on the {position} of P{owner}'s library."
                    )));
                    events.push(permanent_moved_event(
                        &self.state,
                        oid,
                        owner,
                        rv1::permanent_moved::Destination::Library,
                    ));
                }
                _ => {
                    return Err(EngineError::ProgramInvariant(
                        "ChooseCard destination escaped model validation",
                    ));
                }
            }
        }
        let previous = stack.previous_result.clone();
        events.push(ev_log(format!("P{chooser} chooses {card_label}.")));
        self.complete_parked_stack_with_previous(stack, previous, events)
    }

    pub(super) fn finish_different_named_graveyard_return(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, generations, spell_label) = match &pending.continuation {
            ResolutionContinuation::DifferentNamedGraveyardReturn {
                stack,
                candidate_generations,
                spell_label,
            } => (
                stack.clone(),
                candidate_generations.clone(),
                spell_label.clone(),
            ),
            _ => {
                return Err(EngineError::Illegal(
                    "different-name graveyard-return continuation missing",
                ))
            }
        };
        let controller = stack.item.controller;
        let mut events = Vec::new();
        if chosen.is_empty() {
            events.push(ev_log(format!(
                "P{controller} chooses no permanent cards to return."
            )));
            return self.complete_parked_resolution_with_previous(
                stack.item,
                stack.resume_effect_index,
                stack.previous_result,
                events,
            );
        }

        let mut entries = Vec::with_capacity(chosen.len());
        for &oid in chosen {
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
                || !self.state.objects.get(&oid).is_some_and(|object| {
                    object.zone == Zone::Graveyard && object.owner == controller
                })
            {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "graveyard permanent choice became stale",
                ));
            }
            entries.push(super::super::resolution::zones::plain_return_entry(
                self, oid, controller,
            ));
        }

        let Some(stack) = self.begin_zone_entry_batch(
            stack,
            entries,
            Zone::Graveyard,
            &spell_label,
            None,
            &mut events,
        )?
        else {
            return Ok(finish_with_events(self, events));
        };
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            stack.previous_result,
            events,
        )
    }

    pub(super) fn finish_linked_exile_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (mut stack, key, action, generations, spell_label) = match &pending.continuation {
            ResolutionContinuation::LinkedExileChoice {
                stack,
                key,
                action,
                candidate_generations,
                spell_label,
            } => (
                stack.clone(),
                key.clone(),
                *action,
                candidate_generations.clone(),
                spell_label.clone(),
            ),
            _ => {
                return Err(EngineError::Illegal(
                    "linked-Exile-choice continuation missing",
                ))
            }
        };
        let controller = stack.item.controller;
        let mut events = Vec::new();
        let Some(&object_id) = chosen.first() else {
            if matches!(action, PendingLinkedExileAction::Play { .. }) {
                if let Some(cohort) = self
                    .state
                    .pending_linked_exile_play
                    .as_mut()
                    .filter(|cohort| cohort.stack.item.id == stack.item.id && cohort.key == key)
                {
                    // Declining the next-card prompt ends this occurrence. The persistent linked
                    // record remains available to later trigger occurrences.
                    cohort.remaining.clear();
                }
            }
            events.push(ev_log(format!(
                "P{controller} declines the linked Exile choice."
            )));
            return self.complete_parked_resolution_with_previous(
                stack.item,
                stack.resume_effect_index,
                crate::state::EffectResult::default(),
                events,
            );
        };
        let expected_generation = generations
            .iter()
            .find_map(|(candidate, generation)| (*candidate == object_id).then_some(*generation));
        let current_generation = self
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0);
        let linked_current = self
            .state
            .linked_exile_records
            .get(&key)
            .is_some_and(|records| {
                records.iter().any(|record| {
                    record.object_id == object_id
                        && record.zone_change_generation == current_generation
                })
            });
        let Some(object) = self.state.objects.get(&object_id) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("linked Exile choice became stale"));
        };
        if expected_generation != Some(current_generation)
            || object.zone != Zone::Exile
            || !linked_current
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("linked Exile choice became stale"));
        }
        let owner = object.owner;
        let card_label = object_display_name(&self.state, self.registry, object_id);
        let receipt = super::super::payment::card_result_entry(
            &self.state,
            self.registry,
            tricerules_card_model::primitives::CardResultAction::ChooseLinkedExiledCard,
            owner,
            object_id,
        );
        let previous_result = crate::state::EffectResult {
            cards: vec![receipt],
            ..crate::state::EffectResult::default()
        };
        if matches!(action, PendingLinkedExileAction::Play { .. }) {
            if let Some(cohort) = self
                .state
                .pending_linked_exile_play
                .as_mut()
                .filter(|cohort| cohort.stack.item.id == stack.item.id && cohort.key == key)
            {
                cohort.in_flight = Some(crate::state::TriggerObjectRef {
                    object_id,
                    zone_change_generation: current_generation,
                    controller_at_event: controller,
                });
                cohort.remaining.retain(|candidate| {
                    candidate.object_id != object_id
                        || candidate.zone_change_generation != current_generation
                });
            } else {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("linked play cohort disappeared"));
            }
        }
        match action {
            PendingLinkedExileAction::Move {
                destination:
                    tricerules_card_model::primitives::LinkedExiledCardDestination::OwnerGraveyard,
                ..
            } => {
                self.commit_observed_zone_move(object_id, Zone::Graveyard, None, &mut events)?;
                events.push(ev_log(format!(
                    "{spell_label} moves {card_label} to its owner's graveyard."
                )));
                events.push(permanent_moved_event(
                    &self.state,
                    object_id,
                    owner,
                    rv1::permanent_moved::Destination::Graveyard,
                ));
                self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    previous_result,
                    events,
                )
            }
            PendingLinkedExileAction::Move {
                destination: tricerules_card_model::primitives::LinkedExiledCardDestination::ControllerBattlefield,
                ..
            } => {
                stack.previous_result = previous_result;
                let entry = BattlefieldEntryEvent {
                    entry_reveal_receipts: Vec::new(),
                    mana_colors_spent_to_cast: Default::default(),
                    prepared: false,
                    object_id,
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
                if let Some(completed) = self.begin_zone_entry_batch(
                    stack,
                    vec![entry],
                    Zone::Exile,
                    &spell_label,
                    None,
                    &mut events,
                )? {
                    self.complete_parked_resolution_with_previous(
                        completed.item,
                        completed.resume_effect_index,
                        completed.previous_result,
                        events,
                    )
                } else {
                    Ok(finish_with_events(self, events))
                }
            }
            PendingLinkedExileAction::Play {
                kind,
                return_source_to_owner_hand_if_cast,
            } => {
                let permission_scope = match kind {
                    tricerules_card_model::primitives::LinkedExiledCardPlayKind::Cast => {
                        ExilePlayPermissionScope::CastCard
                    }
                    tricerules_card_model::primitives::LinkedExiledCardPlayKind::Play => {
                        ExilePlayPermissionScope::PlayCard
                    }
                };
                let prompt = match kind {
                    tricerules_card_model::primitives::LinkedExiledCardPlayKind::Cast => {
                        format!("P{controller}: cast {card_label} without paying its mana cost?")
                    }
                    tricerules_card_model::primitives::LinkedExiledCardPlayKind::Play => {
                        format!("P{controller}: play {card_label} without paying its mana cost?")
                    }
                };
self.state.pending_resolution = Some(PendingResolution {
                    resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
                    resolution_prompt_semantics: None,

            deciding_player: controller,
                    presentation: PendingResolutionPresentation {
                        source_object_id: stack.item.id,
                        candidates: vec![object_id],
                        min: 0,
                        max: 1,
                        ordered: false,
                        prompt: prompt.clone(),
                        choice_kind: rv1::ChoiceKind::SpecialCast,
                        unique_names: false,
                    },
                    continuation: ResolutionContinuation::SpecialCast {
                        stack,
                        exiled: TriggerObjectRef {
                            object_id,
                            zone_change_generation: current_generation,
                            controller_at_event: controller,
                        },
                        face_index: 0,
                        method: SpellCastMethod::Permission,
                        cost: ManaCost::default(),
                        undo_history_start: self.state.undoable_mana_abilities.len(),
                        permission_scope: Some(permission_scope),
                        return_source_to_owner_hand_if_cast,
                        max_face_mana_value: None,
                        completion: SpecialCastCompletion::ResumeStack,
                    },
                });
                events.push(
                    self.resolution_payment_choice_event()
                        .expect("linked exile play offer"),
                );
                events.push(ev_log(prompt));
                Ok(finish_with_events(self, events))
            }
        }
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

    const MILL_CHOOSE_THEN_DRAW: &str = r#"(
        id: "mill_choose_then_draw_fixture",
        name: "Mill Choose Then Draw Fixture",
        face_id: "mill_choose_then_draw_fixture",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_program: Some((body: (steps: [
            (id: "mill", bind: ["milled"], kind: Perform(Mill(count: 1, who: Controller))),
            (id: "recover", input: Some("milled"), kind: Perform(ChooseCard(
                source: CardResultCohort(players: Controller),
                filter: (card_type: Some(Creature)),
                destination: Hand,
                optional: true,
            ))),
            (id: "draw", kind: Perform(Draw(who: Controller, count: 1))),
        ], yields: []), target_bindings: [])),
    )"#;

    const MILL_CHOOSE_ONLY: &str = r#"(
        id: "mill_choose_only_fixture",
        name: "Mill Choose Only Fixture",
        face_id: "mill_choose_only_fixture",
        mana_cost: "{0}",
        types: ["Instant"],
        spell_program: Some((body: (steps: [
            (id: "mill", bind: ["milled"], kind: Perform(Mill(count: 1, who: Controller))),
            (id: "recover", input: Some("milled"), kind: Perform(ChooseCard(
                source: CardResultCohort(players: Controller),
                filter: (card_type: Some(Creature)),
                destination: Hand,
                optional: false,
            ))),
        ], yields: []), target_bindings: [])),
    )"#;

    const OPPONENT_MILL_CHOOSE_ONLY: &str = r#"(
        id: "opponent_mill_choose_only_fixture",
        name: "Opponent Mill Choose Only Fixture",
        face_id: "opponent_mill_choose_only_fixture",
        mana_cost: "",
        types: ["Artifact"],
        triggered_abilities: [(
            ability_id: "triggered_01",
            presentation: OracleLines([1]),
            trigger: AtBeginningOfDrawStep(player: AnyPlayer),
            effect: [],
            program: Some((body: (steps: [
                (id: "mill", bind: ["milled"], kind: Perform(Mill(count: 1, who: AffectedPlayer))),
                (id: "recover", input: Some("milled"), kind: Perform(ChooseCard(
                    source: CardResultCohort(players: Opponents),
                    filter: (card_type: Some(Creature)),
                    destination: Hand,
                    optional: false,
                ))),
            ], yields: []), target_bindings: [])),
        )],
    )"#;

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
            prototyped: false,
            mana_colors_spent_to_cast: Default::default(),
            id: 90_001,
            controller: 0,
            card_id: "cultivate".into(),
            targets: Vec::new(),
            ability_text: None,
            source_permanent_id: None,
            source_owner: Some(0),
            source_zone_change: 0,
            trigger_use_key: None,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            activated_ability_occurrence: None,
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

    fn choose_card_pending(
        candidate: crate::state::CardChoiceCandidateRef,
        destination: tricerules_card_model::primitives::GraveyardDestination,
    ) -> PendingResolution {
        use crate::state::{
            ParkedStackResolution, PendingResolutionPresentation, ResolutionContinuation,
        };
        let mut item = test_stack_item();
        item.card_id = "divination".into();
        let mut stack = ParkedStackResolution::new(item);
        stack.resume_effect_index = Some(0);
        PendingResolution {
            deciding_player: 0,
            presentation: PendingResolutionPresentation {
                source_object_id: stack.item.id,
                candidates: vec![candidate.object_id],
                min: 1,
                max: 1,
                ordered: false,
                prompt: "choose the publicly milled card".into(),
                choice_kind: custom::ChoiceKind::ZoneSearch,
                unique_names: false,
            },
            continuation: ResolutionContinuation::ChooseCard {
                stack,
                chooser: 0,
                destination,
                candidates: vec![candidate],
                spell_label: "Mill fixture".into(),
            },
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,
        }
    }

    fn fixture_stack_spell(engine: &GameEngine, oid: ObjectId) -> StackItem {
        let object = &engine.state.objects[&oid];
        let generation = engine.state.zone_change_generation[&oid];
        let mut item = test_stack_item();
        item.id = oid;
        item.controller = object.controller;
        item.card_id = object.card_id.clone();
        item.face_index = object.face_up_index;
        item.prototyped = object.prototyped;
        item.ability_text = None;
        item.ability_index = None;
        item.activated_ability = None;
        item.activated_ability_occurrence = None;
        item.is_copy = false;
        item.cast_occurrence = Some(crate::state::StackObjectRef {
            object_id: oid,
            zone_change_generation: Some(generation),
        });
        item
    }

    #[test]
    fn choose_card_moves_exact_candidates_from_each_public_source_to_owner_destination() {
        use crate::state::CardChoiceCandidateRef;
        use tricerules_card_model::primitives::GraveyardDestination;

        let cases = [
            (Zone::Graveyard, GraveyardDestination::Hand),
            (Zone::Battlefield, GraveyardDestination::LibraryTop),
            (Zone::Exile, GraveyardDestination::LibraryBottom),
            (Zone::Stack, GraveyardDestination::Hand),
            (Zone::Command, GraveyardDestination::LibraryTop),
        ];
        for (index, (source_zone, destination)) in cases.into_iter().enumerate() {
            let mut engine = GameEngine::new(
                tricerules_cards::registry::global(),
                90_130 + index as u64,
                &[0, 1],
                20,
                Some(vec![
                    vec!["grizzly_bears".to_string(); 12],
                    vec!["island".to_string(); 12],
                ]),
                true,
            )
            .expect("new game");
            let oid = *engine.state.players[0]
                .library
                .front()
                .expect("fixture candidate in library");
            let mut setup_events = Vec::new();
            engine
                .commit_observed_zone_move(oid, source_zone, None, &mut setup_events)
                .expect("move candidate into public source");
            let same_name_stack_sibling = if source_zone == Zone::Stack {
                let sibling = *engine.state.players[0]
                    .library
                    .front()
                    .expect("same-name stack sibling in library");
                engine
                    .commit_observed_zone_move(sibling, Zone::Stack, None, &mut setup_events)
                    .expect("move exact same-name sibling to Stack");
                let sibling_item = fixture_stack_spell(&engine, sibling);
                engine.state.stack.push(sibling_item);
                Some(sibling)
            } else {
                None
            };
            if source_zone == Zone::Stack {
                let candidate_item = fixture_stack_spell(&engine, oid);
                engine.state.stack.push(candidate_item);
            }
            let source_generation = engine.state.zone_change_generation[&oid];
            let candidate = CardChoiceCandidateRef {
                object_id: oid,
                zone_change_generation: source_generation,
                source_zone,
            };
            let hand_before = engine.state.players[0].hand.len();
            let pending = choose_card_pending(candidate, destination);
            let batch = engine
                .finish_choose_card(pending, &[oid])
                .unwrap_or_else(|error| panic!("{source_zone:?} choice failed: {error:?}"));
            let move_destination = match destination {
                GraveyardDestination::Hand => Zone::Hand,
                GraveyardDestination::LibraryTop | GraveyardDestination::LibraryBottom => {
                    Zone::Library
                }
                _ => panic!("unexpected fixture destination"),
            };
            // The authored tail draws two cards. A LibraryTop destination must put this exact
            // candidate first, so it is then drawn by the first tail instruction.
            let final_zone = if destination == GraveyardDestination::LibraryTop {
                Zone::Hand
            } else {
                move_destination
            };
            assert_eq!(engine.state.objects[&oid].owner, 0);
            assert_eq!(engine.state.objects[&oid].zone, final_zone);
            assert_eq!(
                engine.state.players[0].hand.len(),
                hand_before
                    + if destination == GraveyardDestination::Hand {
                        1
                    } else {
                        0
                    }
                    + 2,
                "the parked Divination tail must draw exactly two cards after {source_zone:?}"
            );
            assert_eq!(
                engine.state.zone_change_generation[&oid],
                source_generation
                    + if destination == GraveyardDestination::LibraryTop {
                        2
                    } else {
                        1
                    },
                "source, destination, and any top-card draw must each advance generation once"
            );
            let owner = &engine.state.players[0];
            match final_zone {
                Zone::Hand => assert!(owner.hand.contains(&oid)),
                Zone::Library => {
                    if destination == GraveyardDestination::LibraryTop {
                        assert_eq!(owner.library.front(), Some(&oid));
                    } else {
                        assert_eq!(owner.library.back(), Some(&oid));
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                batch.events.iter().any(|event| matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                        if moved.object_id == oid
                            && moved.destination == match move_destination {
                                Zone::Hand => rv1::permanent_moved::Destination::Hand as i32,
                                Zone::Library => rv1::permanent_moved::Destination::Library as i32,
                                _ => unreachable!(),
                            }
                )),
                "missing exact PermanentMoved for {source_zone:?}"
            );
            assert!(
                !batch.events.iter().any(|event| matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::StackResolved(resolved)) if resolved.object_id == oid
                )),
                "moving a Stack candidate is not resolving it"
            );
            if source_zone == Zone::Stack {
                assert!(!engine.state.stack.iter().any(|item| item.id == oid));
                assert!(!engine.state.stack_presentations.contains_key(&oid));
                let sibling = same_name_stack_sibling.expect("same-name sibling fixture");
                assert_eq!(
                    engine.state.objects[&sibling].card_id,
                    engine.state.objects[&oid].card_id
                );
                assert_eq!(engine.state.objects[&sibling].zone, Zone::Stack);
                assert!(engine.state.stack.iter().any(|item| item.id == sibling));
            }
            let current = format!("{:?}", engine.state);
            let duplicate = engine.apply_scenario_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![oid],
                            ..Default::default()
                        },
                    )),
                },
            );
            assert!(
                duplicate.is_err(),
                "a completed answer must not be reusable"
            );
            assert!(
                format!("{:?}", engine.state) == current,
                "a duplicate answer changed completed state"
            );
        }
    }

    #[test]
    fn opponent_mill_cohort_uses_generic_public_zone_choice_and_returns_to_owner() {
        let registry = Box::leak(Box::new(
            tricerules_cards::registry::CardRegistry::from_chunks_and_tokens(
                &[
                    OPPONENT_MILL_CHOOSE_ONLY,
                    include_str!("../../../../tricerules-cards/data/island.ron"),
                    include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
                ],
                &[],
            )
            .expect("opponent Mill -> ChooseCard fixture compiles"),
        ));
        let mut engine = GameEngine::new(
            registry,
            90_137,
            &[0, 1],
            20,
            Some(vec![
                vec!["island".to_string(); 12],
                vec!["island".to_string(); 12],
            ]),
            true,
        )
        .expect("fixture game");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;

        let source = battlefield_card(&mut engine, "opponent_mill_choose_only_fixture");
        let candidate = *engine.state.players[1]
            .library
            .front()
            .expect("opponent top card is available to mill");
        engine.state.objects.get_mut(&candidate).unwrap().card_id = "grizzly_bears".into();
        let face = registry
            .get("opponent_mill_choose_only_fixture")
            .and_then(|definition| definition.face(0))
            .expect("authored fixture face");
        let ability = face
            .triggered_abilities
            .first()
            .expect("authored opponent draw-step trigger")
            .clone();
        let ability_definition = tricerules_card_model::AbilityDefinitionId {
            card_id: "opponent_mill_choose_only_fixture".into(),
            face_id: face.face_id.clone(),
            ability_path: vec![ability.ability_id.clone()],
        };
        let ability_owner =
            tricerules_card_model::ProgramOwnerId::ability(ability_definition.clone());
        assert!(registry
            .compiled_program_for_owner(&ability_owner)
            .is_some());
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut top = test_stack_item();
        top.id = 90_138;
        top.card_id = "opponent_mill_choose_only_fixture".into();
        top.controller = 0;
        top.ability_text = Some("opponent mill fixture".into());
        top.source_permanent_id = Some(source);
        top.source_owner = Some(0);
        top.source_zone_change = source_generation;
        top.trigger_use_key = Some(crate::state::TriggerUseKey {
            object_id: source,
            zone_change_generation: source_generation,
            ability_origin: crate::state::TriggerAbilityOrigin::Printed(ability_definition),
        });
        top.triggered_ability = Some(ability);
        top.is_triggered = true;
        top.is_copy = false;
        top.trigger_context.affected_player = Some(1);
        let mut events = Vec::new();
        engine.state.stack.push(top);

        engine
            .resolve_top_of_stack(&mut events)
            .expect("opponent is milled and its result cohort is offered");
        engine
            .publish_resolution_prompt_ids(&mut events)
            .expect("publish choice correlation");
        let choice = events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .expect("opponent-cohort choice offer");
        assert_eq!(choice.choice_kind, rv1::ChoiceKind::ZoneSearch as i32);
        assert_eq!(choice.candidate_object_ids, [candidate]);
        assert_eq!(
            choice.candidate_source_zones,
            [rv1::ChoiceCandidateSourceZone::Graveyard as i32]
        );
        assert_eq!(engine.state.objects[&candidate].owner, 1);
        assert_eq!(engine.state.objects[&candidate].zone, Zone::Graveyard);
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("choice remains pending");
        let source_generation = engine.state.zone_change_generation[&candidate];
        assert!(matches!(
            &pending.continuation,
            ResolutionContinuation::ChooseCard { candidates, chooser: 0, .. }
                if candidates == &[crate::state::CardChoiceCandidateRef {
                    object_id: candidate,
                    zone_change_generation: source_generation,
                    source_zone: Zone::Graveyard,
                }]
        ));
        let prompt_id = pending.resolution_prompt_id.get();

        engine
            .submit_resolution_choice(
                0,
                &rv1::SubmitResolutionChoice {
                    resolution_prompt_id: prompt_id,
                    chosen_object_ids: vec![candidate],
                    ..Default::default()
                },
            )
            .expect("chooser selects the exact opponent-owned object");

        assert_eq!(engine.state.objects[&candidate].owner, 1);
        assert_eq!(engine.state.objects[&candidate].zone, Zone::Hand);
        assert!(engine.state.players[1].hand.contains(&candidate));
        assert!(!engine.state.players[0].hand.contains(&candidate));
        assert_eq!(
            engine.state.zone_change_generation[&candidate],
            source_generation + 1
        );
        assert!(engine.state.pending_resolution.is_none());
    }

    #[test]
    fn mill_choose_program_resumes_its_tail_exactly_once_after_the_selected_movement() {
        let registry = Box::leak(Box::new(
            tricerules_cards::registry::CardRegistry::from_chunks_and_tokens(
                &[
                    MILL_CHOOSE_THEN_DRAW,
                    include_str!("../../../../tricerules-cards/data/island.ron"),
                    include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
                ],
                &[],
            )
            .expect("Mill -> ChooseCard -> Draw fixture compiles"),
        ));
        let mut engine = GameEngine::new(
            registry,
            90_135,
            &[0, 1],
            20,
            Some(vec![
                vec!["mill_choose_then_draw_fixture".to_string(); 12],
                vec!["island".to_string(); 12],
            ]),
            true,
        )
        .expect("fixture game");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;

        let candidate = *engine.state.players[0]
            .library
            .front()
            .expect("top card is available to mill");
        engine.state.objects.get_mut(&candidate).unwrap().card_id = "grizzly_bears".into();
        let draw_card = *engine.state.players[0]
            .library
            .get(1)
            .expect("a card remains for the program tail");
        engine.state.objects.get_mut(&draw_card).unwrap().card_id = "island".into();
        let spell = *engine.state.players[0]
            .hand
            .first()
            .expect("fixture spell in hand");
        let mut events = Vec::new();
        engine
            .commit_observed_zone_move(spell, Zone::Stack, None, &mut events)
            .expect("cast occurrence enters Stack");
        engine.state.stack.push(fixture_stack_spell(&engine, spell));

        engine
            .resolve_top_of_stack(&mut events)
            .expect("Mill parks at the exact ChooseCard instruction");
        engine
            .publish_resolution_prompt_ids(&mut events)
            .expect("publish the authored choice correlation");
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the authored program offers its mill cohort");
        assert_eq!(pending.presentation.candidates, vec![candidate]);
        assert!(matches!(
            &pending.continuation,
            ResolutionContinuation::ChooseCard { stack, .. } if stack.program.is_some()
        ));
        assert_eq!(engine.state.objects[&candidate].zone, Zone::Graveyard);
        let before_hand = engine.state.players[0].hand.len();
        let before_library = engine.state.players[0].library.len();
        let before_draws = engine.state.turn_history.current.player(0).cards_drawn;
        let source_generation = engine.state.zone_change_generation[&candidate];

        let answer = rv1::SubmitResolutionChoice {
            chosen_object_ids: vec![candidate],
            ..Default::default()
        };
        engine
            .apply_scenario_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        answer.clone(),
                    )),
                },
            )
            .expect("take the selected card and resume the program tail");

        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.objects[&candidate].zone, Zone::Hand);
        assert!(engine.state.players[0].hand.contains(&candidate));
        assert_eq!(engine.state.players[0].hand.len(), before_hand + 2);
        assert_eq!(engine.state.players[0].library.len(), before_library - 1);
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            before_draws + 1
        );
        assert_eq!(
            engine.state.zone_change_generation[&candidate],
            source_generation + 1
        );
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert!(engine.state.stack.is_empty());

        let completed_state = format!("{:?}", engine.state);
        assert!(engine
            .apply_scenario_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(answer)),
                },
            )
            .is_err());
        assert_eq!(format!("{:?}", engine.state), completed_state);
    }

    #[test]
    fn declining_optional_mill_choice_resumes_its_tail_without_repeating_mill() {
        let registry = Box::leak(Box::new(
            tricerules_cards::registry::CardRegistry::from_chunks_and_tokens(
                &[
                    MILL_CHOOSE_THEN_DRAW,
                    include_str!("../../../../tricerules-cards/data/island.ron"),
                    include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
                ],
                &[],
            )
            .expect("optional Mill -> ChooseCard -> Draw fixture compiles"),
        ));
        let mut engine = GameEngine::new(
            registry,
            90_136,
            &[0, 1],
            20,
            Some(vec![
                vec!["mill_choose_then_draw_fixture".to_string(); 12],
                vec!["island".to_string(); 12],
            ]),
            true,
        )
        .expect("fixture game");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;

        let candidate = *engine.state.players[0]
            .library
            .front()
            .expect("top card is available to mill");
        engine.state.objects.get_mut(&candidate).unwrap().card_id = "grizzly_bears".into();
        let draw_card = *engine.state.players[0]
            .library
            .get(1)
            .expect("a card remains for the program tail");
        engine.state.objects.get_mut(&draw_card).unwrap().card_id = "island".into();
        let spell = *engine.state.players[0]
            .hand
            .first()
            .expect("fixture spell in hand");
        let mut events = Vec::new();
        engine
            .commit_observed_zone_move(spell, Zone::Stack, None, &mut events)
            .expect("cast occurrence enters Stack");
        engine.state.stack.push(fixture_stack_spell(&engine, spell));

        engine
            .resolve_top_of_stack(&mut events)
            .expect("Mill parks at the optional ChooseCard instruction");
        engine
            .publish_resolution_prompt_ids(&mut events)
            .expect("publish the optional choice correlation");
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the authored program offers its mill cohort");
        assert_eq!(pending.presentation.candidates, vec![candidate]);
        assert_eq!((pending.presentation.min, pending.presentation.max), (0, 1));
        assert_eq!(engine.state.objects[&candidate].zone, Zone::Graveyard);
        let source_generation = engine.state.zone_change_generation[&candidate];
        let before_hand = engine.state.players[0].hand.len();
        let before_library = engine.state.players[0].library.len();
        let before_draws = engine.state.turn_history.current.player(0).cards_drawn;
        let decline = rv1::SubmitResolutionChoice {
            decision: rv1::ResolutionChoiceDecision::Unspecified as i32,
            ..Default::default()
        };

        engine
            .apply_scenario_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        decline.clone(),
                    )),
                },
            )
            .expect("decline the optional candidate and resume the program tail");

        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.objects[&candidate].zone, Zone::Graveyard);
        assert_eq!(
            engine.state.zone_change_generation[&candidate],
            source_generation
        );
        assert_eq!(engine.state.objects[&draw_card].zone, Zone::Hand);
        assert!(engine.state.players[0].hand.contains(&draw_card));
        assert_eq!(engine.state.players[0].hand.len(), before_hand + 1);
        assert_eq!(engine.state.players[0].library.len(), before_library - 1);
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            before_draws + 1
        );
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert!(engine.state.stack.is_empty());

        let completed_state = format!("{:?}", engine.state);
        assert!(engine
            .apply_scenario_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(decline)),
                },
            )
            .is_err());
        assert_eq!(format!("{:?}", engine.state), completed_state);
    }

    #[test]
    fn choose_card_commander_903_9b_choice_belongs_to_owner_not_chooser() {
        use crate::state::CardChoiceCandidateRef;
        use tricerules_card_model::primitives::GraveyardDestination;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            90_140,
            &[0, 1],
            20,
            Some(vec![
                vec!["grizzly_bears".to_string(); 10],
                vec!["grizzly_bears".to_string(); 10],
            ]),
            true,
        )
        .expect("new game");
        let oid = *engine.state.players[1]
            .library
            .front()
            .expect("owner's commander in library");
        let input_generation = engine
            .state
            .zone_change_generation
            .get(&oid)
            .copied()
            .unwrap_or(0);
        let mut events = Vec::new();
        engine
            .commit_observed_zone_move(oid, Zone::Graveyard, None, &mut events)
            .expect("commander leaves library");
        engine.state.objects.get_mut(&oid).unwrap().card_id = "saskia_the_unyielding".into();
        engine.state.players[1]
            .declared_commander_object_ids
            .push(oid);
        let source_generation = engine.state.zone_change_generation[&oid];
        let candidate = CardChoiceCandidateRef {
            object_id: oid,
            zone_change_generation: source_generation,
            source_zone: Zone::Graveyard,
        };
        let pending = choose_card_pending(candidate, GraveyardDestination::Hand);
        let prompt = engine
            .finish_choose_card(pending, &[oid])
            .expect("commander owner decision is offered");
        let offer = prompt
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .expect("commander owner prompt");
        assert_eq!(offer.deciding_player_id, 1);
        assert!(matches!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| pending.continuation.clone()),
            Some(ResolutionContinuation::ChooseCardCommander {
                chooser: 0,
                commander_owner: 1,
                ..
            })
        ));

        let pending = engine
            .state
            .pending_resolution
            .take()
            .expect("owner prompt pending");
        let before = engine.state.zone_change_generation[&oid];
        let completed = engine
            .finish_choose_card_commander(
                pending,
                &rv1::SubmitResolutionChoice {
                    selected_branch_index: 0,
                    ..Default::default()
                },
                rv1::ResolutionChoiceDecision::SelectBranch,
            )
            .expect("commander owner chooses the command zone");
        assert_eq!(engine.state.objects[&oid].owner, 1);
        assert_eq!(engine.state.objects[&oid].zone, Zone::Command);
        assert!(engine.state.players[1].command_zone.contains(&oid));
        assert!(!engine.state.players[0].command_zone.contains(&oid));
        assert_eq!(engine.state.zone_change_generation[&oid], before + 1);
        assert!(completed.events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                if moved.object_id == oid
                    && moved.destination == rv1::permanent_moved::Destination::Command as i32
        )));
        assert_ne!(input_generation, source_generation);
    }

    #[test]
    fn choose_card_commander_already_in_command_zone_keeps_one_exact_owner_bucket_entry() {
        use crate::state::CardChoiceCandidateRef;
        use tricerules_card_model::primitives::GraveyardDestination;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            90_145,
            &[0, 1],
            20,
            Some(vec![
                vec!["grizzly_bears".to_string(); 10],
                vec!["grizzly_bears".to_string(); 10],
            ]),
            true,
        )
        .expect("new game");
        let oid = *engine.state.players[1]
            .library
            .front()
            .expect("owner's commander starts in library");
        engine.state.objects.get_mut(&oid).unwrap().card_id = "saskia_the_unyielding".into();
        engine.state.players[1]
            .declared_commander_object_ids
            .push(oid);
        let mut events = Vec::new();
        engine
            .commit_observed_zone_move(oid, Zone::Command, None, &mut events)
            .expect("commander enters its owner's command zone");
        let command_generation = engine.state.zone_change_generation[&oid];
        let candidate = CardChoiceCandidateRef {
            object_id: oid,
            zone_change_generation: command_generation,
            source_zone: Zone::Command,
        };

        let prompt = engine
            .finish_choose_card(
                choose_card_pending(candidate, GraveyardDestination::Hand),
                &[oid],
            )
            .expect("a commander currently in Command may be chosen");
        let offer = prompt
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .expect("owner decides where the selected commander goes");
        assert_eq!(offer.deciding_player_id, 1);
        let pending = engine
            .state
            .pending_resolution
            .take()
            .expect("owner prompt pending");
        engine
            .finish_choose_card_commander(
                pending,
                &rv1::SubmitResolutionChoice {
                    selected_branch_index: 0,
                    ..Default::default()
                },
                rv1::ResolutionChoiceDecision::SelectBranch,
            )
            .expect("owner leaves commander in the command zone");

        assert_eq!(engine.state.objects[&oid].zone, Zone::Command);
        assert_eq!(engine.state.objects[&oid].owner, 1);
        assert_eq!(
            engine.state.players[1]
                .command_zone
                .iter()
                .filter(|candidate| **candidate == oid)
                .count(),
            1
        );
        assert!(!engine.state.players[0].command_zone.contains(&oid));
        assert_eq!(
            engine.state.zone_change_generation[&oid],
            command_generation + 1
        );
        assert!(
            !super::super::super::resolution::zones::live_choose_card_candidate(
                &engine,
                oid,
                command_generation,
                Zone::Command,
            )
        );
    }

    #[test]
    fn empty_bound_mill_cohort_does_not_fall_back_to_an_eligible_graveyard_card() {
        let registry = Box::leak(Box::new(
            tricerules_cards::registry::CardRegistry::from_chunks_and_tokens(
                &[
                    MILL_CHOOSE_ONLY,
                    include_str!("../../../../tricerules-cards/data/island.ron"),
                    include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
                ],
                &[],
            )
            .expect("Mill -> bound ChooseCard fixture compiles"),
        ));
        let mut engine = GameEngine::new(
            registry,
            90_146,
            &[0, 1],
            20,
            Some(vec![
                vec!["mill_choose_only_fixture".to_string(); 12],
                vec!["island".to_string(); 12],
            ]),
            true,
        )
        .expect("fixture game");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;

        let library = std::mem::take(&mut engine.state.players[0].library);
        let prior_graveyard_card = *library.front().expect("fixture library is nonempty");
        engine
            .state
            .objects
            .get_mut(&prior_graveyard_card)
            .unwrap()
            .card_id = "grizzly_bears".into();
        let mut setup_events = Vec::new();
        for oid in library {
            engine
                .commit_observed_zone_move(oid, Zone::Graveyard, None, &mut setup_events)
                .expect("move every preexisting library card out");
        }
        assert!(engine.state.players[0].library.is_empty());
        assert!(engine.state.players[0]
            .graveyard
            .contains(&prior_graveyard_card));
        assert!(engine
            .characteristics(prior_graveyard_card)
            .unwrap()
            .has_type("Creature"));

        let spell = *engine.state.players[0]
            .hand
            .first()
            .expect("fixture spell in hand");
        let mut events = Vec::new();
        engine
            .commit_observed_zone_move(spell, Zone::Stack, None, &mut events)
            .expect("spell enters Stack");
        engine.state.stack.push(fixture_stack_spell(&engine, spell));
        engine
            .resolve_top_of_stack(&mut events)
            .expect("Mill with no library output runs an empty bound cohort");

        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.players[0].library.is_empty());
        assert!(engine.state.players[0]
            .graveyard
            .contains(&prior_graveyard_card));
        assert_eq!(
            engine.state.objects[&prior_graveyard_card].zone,
            Zone::Graveyard
        );
        assert!(!events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
        )));
    }

    #[test]
    fn choose_card_battlefield_departure_keeps_ltb_trigger_and_runs_attachment_sba_before_priority()
    {
        use crate::state::{AttachmentRecipient, CardChoiceCandidateRef};
        use tricerules_card_model::primitives::GraveyardDestination;

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            90_150,
            &[0, 1],
            20,
            Some(vec![
                vec!["featherbrained_filcher".to_string(); 12],
                vec!["island".to_string(); 12],
            ]),
            true,
        )
        .expect("new game");
        let candidate = *engine.state.players[0]
            .library
            .front()
            .expect("Filcher in library");
        let mut events = Vec::new();
        engine
            .commit_observed_zone_move(candidate, Zone::Battlefield, None, &mut events)
            .expect("Filcher enters battlefield");
        let aura = *engine.state.players[0]
            .hand
            .first()
            .expect("fixture Aura in hand");
        engine.state.objects.get_mut(&aura).unwrap().card_id = "pacifism".into();
        engine
            .commit_observed_zone_move(aura, Zone::Battlefield, None, &mut events)
            .expect("Aura enters battlefield");
        engine.state.objects.get_mut(&aura).unwrap().attached_to =
            Some(AttachmentRecipient::Object(candidate));
        let generation = engine.state.zone_change_generation[&candidate];
        let choice = CardChoiceCandidateRef {
            object_id: candidate,
            zone_change_generation: generation,
            source_zone: Zone::Battlefield,
        };

        let mut batch = engine
            .finish_choose_card(
                choose_card_pending(choice, GraveyardDestination::Hand),
                &[candidate],
            )
            .expect("move the chosen battlefield creature to its owner's hand");

        assert_eq!(engine.state.objects[&candidate].zone, Zone::Hand);
        assert!(engine.state.players[0].hand.contains(&candidate));
        assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
        assert!(engine.state.players[0].graveyard.contains(&aura));
        assert_eq!(engine.state.objects[&aura].attached_to, None);
        assert!(
            engine.state.staged_trigger_groups.iter().any(|group| {
                group.triggers.iter().any(|trigger| {
                    trigger.source_permanent_id == candidate
                        && trigger.ability.trigger
                            == tricerules_card_model::TriggerCondition::WhenSelfLeavesBattlefield
                })
            }),
            "the LTB event must be staged from the pre-move snapshot"
        );
        let aura_move = batch.events.iter().position(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                if moved.object_id == aura
                    && moved.destination == rv1::permanent_moved::Destination::Graveyard as i32
        )).expect("attachment cleanup is publicly reported");
        let priority_change = batch
            .events
            .iter()
            .position(|event| matches!(&event.ev, Some(rv1::ruled_event::Ev::PriorityChanged(_))))
            .expect("resolution completes before priority");
        assert!(
            aura_move < priority_change,
            "attachment SBA must finish before priority"
        );
        // The public command dispatcher flushes this staged group at the command boundary.
        engine.flush_staged_triggers(&mut batch.events);
        assert!(
            engine.state.stack.iter().any(|item| {
                item.card_id == "featherbrained_filcher"
                    && item.is_triggered
                    && item.source_permanent_id == Some(candidate)
            }),
            "the creature's leaves-the-battlefield trigger must be put on the Stack"
        );
        assert!(batch.events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                if moved.object_id == candidate
                    && moved.destination == rv1::permanent_moved::Destination::Hand as i32
        )));
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.priority_idx, engine.state.active_player_idx);
    }

    #[test]
    fn linked_exile_choice_receipt_drives_later_token_branch_without_a_graveyard_move() {
        use crate::state::{CardResultCohort, CardResultEntry, ParkedStackResolution};
        use tricerules_card_model::primitives::{CardResultAction, CardTypeFilter};

        for (card_type, token_id) in [
            (CardTypeFilter::Land, "treasure"),
            (CardTypeFilter::Nonland, "rogue_b_2_2"),
        ] {
            let mut engine = GameEngine::new(
                tricerules_cards::registry::global(),
                90_041,
                &[0, 1],
                20,
                None,
                true,
            )
            .unwrap();
            engine.state.turn_step = TurnStep::Main1;
            let mut item = test_stack_item();
            item.card_id = "currency_converter".into();
            item.ability_text = Some("return linked card".into());
            item.ability_index = Some(1);
            item.source_permanent_id = Some(90_002);
            item.activated_ability = Some(
                engine
                    .registry
                    .get("currency_converter")
                    .unwrap()
                    .primary_face()
                    .activated_abilities[1]
                    .clone(),
            );

            // The generic suspension path stores an empty result at effect index 1. This fixture
            // models the accepted pre-move receipt replacing it; the selected object is absent
            // and no graveyard movement result exists, as in a future move-replacement outcome.
            let mut parked = ParkedStackResolution::new(item.clone());
            parked.resume_effect_index = Some(1);
            assert!(parked.previous_result.cards.is_empty());
            let selected_object_id = 99_991;
            let previous_result: crate::state::EffectResult = CardResultCohort {
                cards: vec![CardResultEntry {
                    action: CardResultAction::ChooseLinkedExiledCard,
                    affected_player: 0,
                    object_id: selected_object_id,
                    zone_change_generation: 7,
                    matched_card_types: vec![card_type],
                    provenance: crate::state::CardResultProvenance::ActionFacts,
                }],
                program_discard_result: None,
            }
            .into();

            let batch = engine
                .complete_parked_resolution_with_previous(
                    parked.item,
                    parked.resume_effect_index,
                    previous_result,
                    Vec::new(),
                )
                .expect("resume the type-dependent result branch");

            assert!(!engine.state.objects.contains_key(&selected_object_id));
            assert!(engine.state.players[0].graveyard.is_empty());
            let tokens = engine.state.players[0]
                .battlefield
                .iter()
                .filter(|object_id| engine.state.objects[object_id].card_id == token_id)
                .count();
            assert_eq!(
                tokens, 1,
                "saved {card_type:?} snapshot chooses {token_id}; events: {:?}",
                batch.events
            );
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
                    provenance: crate::state::CardResultProvenance::ActionFacts,
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
