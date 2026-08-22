use super::*;

impl GameEngine {
    /// Apply one step of a private top-library partition. Scry sends the selected cohort to the
    /// library bottom; surveil and bounded looks send it to the graveyard. Every kind shares the
    /// same second interrupt for ordering two or more cards retained on top.
    pub(super) fn finish_library_partition(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let controller = pending.deciding_player;
        let Some(idx) = self.state.player_idx(controller) else {
            return Err(EngineError::Illegal("library-partition player missing"));
        };
        let mut ev = vec![];
        let (stack, looked_at, stage, kind) = match &pending.continuation {
            ResolutionContinuation::LibraryPartition {
                stack,
                looked_at,
                stage,
                kind,
            } => (stack.clone(), looked_at.clone(), *stage, *kind),
            _ => {
                return Err(EngineError::Illegal(
                    "library-partition continuation missing",
                ));
            }
        };

        if stage == PendingLibraryPartitionStage::ChooseDestination {
            let remaining: Vec<ObjectId> = looked_at
                .iter()
                .copied()
                .filter(|oid| !chosen.contains(oid))
                .collect();

            match kind {
                PendingLibraryPartitionKind::Scry => {
                    if !chosen.is_empty() {
                        let names = self.object_names(chosen);
                        self.state.players[idx]
                            .library
                            .retain(|oid| !chosen.contains(oid));
                        for &oid in chosen {
                            self.state.players[idx].library.push_back(oid);
                        }
                        let noun = if chosen.len() == 1 { "card" } else { "cards" };
                        ev.push(ev_log(format!(
                            "P{controller} puts {} {noun} on the bottom of their library.",
                            chosen.len()
                        )));
                        ev.push(ev_log_private(
                            format!("P{controller} bottoms {}.", names.join(", ")),
                            controller,
                        ));
                    } else {
                        ev.push(ev_log(format!(
                            "P{controller} keeps every scried card on top."
                        )));
                    }
                }
                PendingLibraryPartitionKind::Surveil | PendingLibraryPartitionKind::Look => {
                    if !chosen.is_empty() {
                        let names = self.object_names(chosen);
                        for &oid in chosen {
                            let owner = self
                                .state
                                .objects
                                .get(&oid)
                                .map(|object| object.owner)
                                .ok_or(EngineError::Illegal("library candidate missing"))?;
                            let source_library_position = self.state.players[idx]
                                .library
                                .iter()
                                .position(|candidate| *candidate == oid)
                                .ok_or(EngineError::Illegal(
                                    "library candidate no longer in library",
                                ))?
                                as u32;
                            move_object_to_zone(
                                &mut self.state,
                                self.registry,
                                oid,
                                Zone::Graveyard,
                                None,
                            )?;
                            ev.push(permanent_moved_event_with_library_position(
                                &self.state,
                                oid,
                                owner,
                                rv1::permanent_moved::Destination::Graveyard,
                                source_library_position,
                            ));
                        }
                        let noun = if chosen.len() == 1 { "card" } else { "cards" };
                        ev.push(ev_log(format!(
                            "P{controller} puts {} {noun} into their graveyard.",
                            chosen.len()
                        )));
                        ev.push(ev_log_private(
                            format!(
                                "P{controller} puts {} into their graveyard.",
                                names.join(", ")
                            ),
                            controller,
                        ));
                    } else {
                        ev.push(ev_log(format!(
                            "P{controller} keeps every looked-at card on top."
                        )));
                    }
                }
            }

            if remaining.len() > 1 {
                return self.park_library_partition_ordering(pending, remaining, ev);
            }
        } else {
            self.state.players[idx]
                .library
                .retain(|oid| !chosen.contains(oid));
            for &oid in chosen {
                self.state.players[idx].library.push_front(oid);
            }
            ev.push(ev_log(format!(
                "P{controller} orders {} cards on top of their library.",
                chosen.len()
            )));
            ev.push(ev_log_private(
                format!(
                    "P{controller} puts {} back on top, in that order.",
                    self.object_names(chosen).join(", ")
                ),
                controller,
            ));
        }

        self.complete_library_partition(stack, controller, kind, ev)
    }

    fn park_library_partition_ordering(
        &mut self,
        pending: PendingResolution,
        remaining: Vec<ObjectId>,
        mut ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let mut pending = pending;
        let controller = pending.deciding_player;
        let source_object_id = pending.presentation.source_object_id;
        let n = remaining.len() as u32;
        let (candidate_card_ids, candidate_names) =
            super::resolution::candidate_identities(self, &remaining);
        let kind = match &pending.continuation {
            ResolutionContinuation::LibraryPartition { kind, .. } => *kind,
            _ => unreachable!("validated library-partition continuation"),
        };
        let label = match kind {
            PendingLibraryPartitionKind::Scry => "Scry",
            PendingLibraryPartitionKind::Surveil => "Surveil",
            PendingLibraryPartitionKind::Look => "Look",
        };
        let prompt = format!(
            "{label}: click the {n} cards staying on top in order — the last one you click is the \
             next card you draw."
        );
        let choice_kind = match kind {
            PendingLibraryPartitionKind::Scry => rv1::ChoiceKind::LibraryTop,
            PendingLibraryPartitionKind::Surveil | PendingLibraryPartitionKind::Look => {
                rv1::ChoiceKind::LibraryLook
            }
        };
        ev.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: controller,
                    source_object_id,
                    prompt_text: prompt.clone(),
                    choice_kind: choice_kind as i32,
                    candidate_object_ids: remaining.clone(),
                    candidate_card_ids,
                    candidate_names,
                    min: n,
                    max: n,
                    ordered: true,
                    unique_names: false,
                    candidate_server_card_ids: Vec::new(),
                    resolution_branches: Vec::new(),
                    mana_cost: String::new(),
                    generic_mana_cost: 0,
                    payment_currently_legal: false,
                    candidate_selectable: match kind {
                        PendingLibraryPartitionKind::Scry => Vec::new(),
                        PendingLibraryPartitionKind::Surveil
                        | PendingLibraryPartitionKind::Look => vec![true; n as usize],
                    },
                    reveal_audience: 0,
                    revealed_zone_owner_player_id: None,
                },
            )),
        });
        ev.push(ev_log_private(prompt.clone(), controller));
        pending.presentation.candidates = remaining;
        pending.presentation.min = n;
        pending.presentation.max = n;
        pending.presentation.ordered = true;
        pending.presentation.prompt = prompt;
        let ResolutionContinuation::LibraryPartition { stage, .. } = &mut pending.continuation
        else {
            unreachable!("validated library-partition continuation")
        };
        *stage = PendingLibraryPartitionStage::OrderTop;
        self.state.pending_resolution = Some(pending);
        Ok(finish_with_events(self, ev))
    }

    fn complete_library_partition(
        &mut self,
        stack: ParkedStackResolution,
        controller: PlayerId,
        kind: PendingLibraryPartitionKind,
        ev: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        if kind == PendingLibraryPartitionKind::Surveil {
            self.fire_triggers(&[GameEvent::Surveilled { player: controller }]);
        }
        self.complete_parked_resolution(stack.item, stack.resume_effect_index, ev)
    }

    /// Finish either step of a bounded library look. Step 0 may move one matching card to hand;
    /// random-order cards finish immediately, while chosen-order cards park one more image-based
    /// ordered pick. Step 1 appends the complete submitted permutation to the library bottom.
    pub(super) fn finish_look_choose_bottom(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let controller = pending.deciding_player;
        let Some(idx) = self.state.player_idx(controller) else {
            return Err(EngineError::Illegal("looking player missing"));
        };
        let mut ev = Vec::new();
        let (stack, stage) = match &pending.continuation {
            ResolutionContinuation::LibraryLook { stack, stage } => (stack.clone(), stage.clone()),
            _ => return Err(EngineError::Illegal("library-look continuation missing")),
        };

        if matches!(stage, PendingLibraryLookStage::OrderBottom) {
            self.state.players[idx]
                .library
                .retain(|oid| !chosen.contains(oid));
            for &oid in chosen {
                self.state.players[idx].library.push_back(oid);
            }
            ev.push(ev_log(format!(
                "P{controller} puts {} cards on the bottom of their library.",
                chosen.len()
            )));
            ev.push(ev_log_private(
                format!(
                    "P{controller} bottoms {}.",
                    self.object_names(chosen).join(", ")
                ),
                controller,
            ));
            return self.complete_parked_resolution(stack.item, stack.resume_effect_index, ev);
        }

        let selected = chosen.first().copied();
        let PendingLibraryLookStage::ChooseToHand {
            looked_at,
            bottom_order,
        } = stage
        else {
            unreachable!("order-bottom returned above")
        };
        let mut remaining: Vec<ObjectId> = looked_at
            .iter()
            .copied()
            .filter(|oid| Some(*oid) != selected)
            .collect();
        if let Some(oid) = selected {
            let name = object_display_name(&self.state, self.registry, oid);
            let owner = self.state.objects[&oid].owner;
            move_object_to_zone(&mut self.state, self.registry, oid, Zone::Hand, None)?;
            ev.push(ev_log(format!("P{controller} reveals {name}.")));
            ev.push(ev_log(format!(
                "P{controller} puts {name} into their hand."
            )));
            ev.push(permanent_moved_event(
                &self.state,
                oid,
                owner,
                rv1::permanent_moved::Destination::Hand,
            ));
        }

        if bottom_order == LibraryBottomOrder::Random {
            shuffle_object_ids_for_current_command(&self.state, controller, &mut remaining);
            self.state.players[idx]
                .library
                .retain(|oid| !remaining.contains(oid));
            for &oid in &remaining {
                self.state.players[idx].library.push_back(oid);
            }
            ev.push(ev_log(format!(
                "P{controller} puts {} cards on the bottom of their library in a random order.",
                remaining.len()
            )));
            ev.push(ev_log_private(
                format!(
                    "P{controller} randomly bottoms {}.",
                    self.object_names(&remaining).join(", ")
                ),
                controller,
            ));
            return self.complete_parked_resolution(stack.item, stack.resume_effect_index, ev);
        }

        if remaining.len() <= 1 {
            self.state.players[idx]
                .library
                .retain(|oid| !remaining.contains(oid));
            for &oid in &remaining {
                self.state.players[idx].library.push_back(oid);
            }
            return self.complete_parked_resolution(stack.item, stack.resume_effect_index, ev);
        }

        let n = remaining.len() as u32;
        let (candidate_card_ids, candidate_names) =
            super::resolution::candidate_identities(self, &remaining);
        let prompt = format!(
            "Click all {n} remaining card images in bottom order. The last image clicked becomes bottom-most."
        );
        ev.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: controller,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::LibraryLook as i32,
                    candidate_object_ids: remaining.clone(),
                    candidate_card_ids,
                    candidate_names,
                    min: n,
                    max: n,
                    ordered: true,
                    unique_names: false,
                    candidate_server_card_ids: Vec::new(),
                    resolution_branches: Vec::new(),
                    mana_cost: String::new(),
                    generic_mana_cost: 0,
                    payment_currently_legal: false,
                    candidate_selectable: vec![true; remaining.len()],
                    reveal_audience: 0,
                    revealed_zone_owner_player_id: None,
                },
            )),
        });
        let mut pending = pending;
        pending.presentation.candidates = std::mem::take(&mut remaining);
        pending.presentation.min = n;
        pending.presentation.max = n;
        pending.presentation.ordered = true;
        pending.presentation.prompt = prompt;
        let ResolutionContinuation::LibraryLook { stage, .. } = &mut pending.continuation else {
            unreachable!("validated library-look continuation")
        };
        *stage = PendingLibraryLookStage::OrderBottom;
        self.state.pending_resolution = Some(pending);
        Ok(finish_with_events(self, ev))
    }
}
