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
        let (stack, looked_at, candidate_generations, stage, kind) = match &pending.continuation {
            ResolutionContinuation::LibraryPartition {
                stack,
                looked_at,
                candidate_generations,
                stage,
                kind,
            } => (
                stack.clone(),
                looked_at.clone(),
                candidate_generations.clone(),
                *stage,
                *kind,
            ),
            _ => {
                return Err(EngineError::Illegal(
                    "library-partition continuation missing",
                ));
            }
        };

        // Library object ids survive zone changes for relay identity, but the looked-at card is
        // the exact physical incarnation captured when this resolution parked (CR 400.7). Check
        // the still-relevant cohort before any chosen card is moved so a stale answer is atomic.
        let cohort = if stage == PendingLibraryPartitionStage::ChooseDestination {
            looked_at.clone()
        } else {
            pending.presentation.candidates.clone()
        };
        if cohort.iter().any(|object_id| {
            let Some(expected_generation) = candidate_generations
                .iter()
                .find_map(|(candidate, generation)| {
                    (*candidate == *object_id).then_some(generation)
                })
                .copied()
            else {
                return true;
            };
            !(self.state.players[idx].library.contains(object_id)
                && self.state.objects.get(object_id).is_some_and(|object| {
                    object.zone == Zone::Library && object.owner == controller
                })
                && self
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0)
                    == expected_generation)
        }) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("stale library-partition cohort"));
        }

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
                    candidate_token_identities: Vec::new(),
                    candidate_player_ids: Vec::new(),
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
                    public_reveal: None,
                    candidate_source_zones: Vec::new(),
                    combat_defender_options: Vec::new(),
                    waterbend: false,
                    selection_slots: Vec::new(),
                    replacement_options: Vec::new(),
                    selection_alternatives: Vec::new(),
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

    /// Finish either step of a bounded library look. Step 0 moves the selected cohort to hand;
    /// random-order cards finish immediately, while chosen-order cards park one more image-based
    /// ordered pick. Step 1 appends the complete submitted permutation to the library bottom.
    pub(super) fn finish_look_choose_bottom(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let controller = pending.deciding_player;
        let Some(idx) = self.state.player_idx(controller) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("looking player missing"));
        };
        let mut ev = Vec::new();
        let (stack, stage) = match &pending.continuation {
            ResolutionContinuation::LibraryLook {
                stack,
                stage,
                candidates,
            } => {
                if candidates.iter().any(|(oid, generation)| {
                    !self.state.players[idx].library.contains(oid)
                        || self.state.objects.get(oid).is_none_or(|object| {
                            object.zone != Zone::Library || object.owner != controller
                        })
                        || self
                            .state
                            .zone_change_generation
                            .get(oid)
                            .copied()
                            .unwrap_or(0)
                            != *generation
                }) {
                    self.state.pending_resolution = Some(pending);
                    return Err(EngineError::Illegal("stale library-look cohort"));
                }
                (stack.clone(), stage.clone())
            }
            _ => return Err(EngineError::Illegal("library-look continuation missing")),
        };

        if matches!(
            stage,
            PendingLibraryLookStage::IntoTheWilds | PendingLibraryLookStage::DeployTheGatewatch
        ) {
            let ResolutionContinuation::LibraryLook { candidates, .. } = &pending.continuation
            else {
                unreachable!("validated library-look continuation")
            };
            let deploy = matches!(stage, PendingLibraryLookStage::DeployTheGatewatch);
            let current_top = !candidates.is_empty()
                && candidates
                    .iter()
                    .map(|(oid, _)| *oid)
                    .eq(self.state.players[idx]
                        .library
                        .iter()
                        .take(candidates.len())
                        .copied())
                && stack.item.controller == controller;
            let legal_accept = chosen.is_empty()
                || (chosen.len() <= if deploy { 2 } else { 1 }
                    && chosen.iter().all(|oid| {
                        candidates.iter().any(|(candidate, _)| candidate == oid)
                            && zone_card_matches_filter(
                                &self.state,
                                self.registry,
                                *oid,
                                Some(&ZoneCardFilter {
                                    card_type: Some(if deploy {
                                        tricerules_cards::primitives::CardTypeFilter::Planeswalker
                                    } else {
                                        tricerules_cards::primitives::CardTypeFilter::Land
                                    }),
                                    ..Default::default()
                                }),
                            )
                    }));
            if !current_top || !legal_accept {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "stale battlefield-look top or eligibility",
                ));
            }
            if chosen.is_empty() && !deploy {
                return self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    stack.previous_result,
                    ev,
                );
            }
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
                    tapped: false,
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
            let completion =
                deploy.then(|| crate::state::ZoneEntryCompletion::DeployRandomBottom {
                    library_owner: controller,
                    looked_refs: candidates.clone(),
                });
            let label = object_display_name(&self.state, self.registry, stack.item.id);
            let Some(stack) = self.begin_zone_entry_batch(
                stack,
                entries,
                Zone::Library,
                &label,
                completion,
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

        let PendingLibraryLookStage::ChooseToHand {
            looked_at,
            bottom_order,
            reveal,
        } = stage
        else {
            unreachable!("order-bottom returned above")
        };
        let mut remaining: Vec<ObjectId> = looked_at
            .iter()
            .copied()
            .filter(|oid| !chosen.contains(oid))
            .collect();
        if reveal && !chosen.is_empty() {
            ev.extend(super::super::reveals::reveal_cards(
                &self.state,
                self.registry,
                chosen,
                stack.item.id,
                &object_display_name(&self.state, self.registry, stack.item.id),
            ));
        }
        for &oid in chosen {
            let name = object_display_name(&self.state, self.registry, oid);
            let owner = self.state.objects[&oid].owner;
            move_object_to_zone(&mut self.state, self.registry, oid, Zone::Hand, None)?;
            let message = format!("P{controller} puts {name} into their hand.");
            ev.push(if reveal {
                ev_log(message)
            } else {
                ev_log_private(message, controller)
            });
            ev.push(permanent_moved_event(
                &self.state,
                oid,
                owner,
                rv1::permanent_moved::Destination::Hand,
            ));
        }
        if !reveal {
            ev.push(ev_log(format!(
                "P{controller} puts {} cards into their hand.",
                chosen.len()
            )));
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
                    candidate_token_identities: Vec::new(),
                    candidate_player_ids: Vec::new(),
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
                    public_reveal: None,
                    candidate_source_zones: Vec::new(),
                    combat_defender_options: Vec::new(),
                    waterbend: false,
                    selection_slots: Vec::new(),
                    replacement_options: Vec::new(),
                    selection_alternatives: Vec::new(),
                },
            )),
        });
        let mut pending = pending;
        pending.presentation.candidates = std::mem::take(&mut remaining);
        pending.presentation.min = n;
        pending.presentation.max = n;
        pending.presentation.ordered = true;
        pending.presentation.prompt = prompt;
        let ResolutionContinuation::LibraryLook {
            stage, candidates, ..
        } = &mut pending.continuation
        else {
            unreachable!("validated library-look continuation")
        };
        *stage = PendingLibraryLookStage::OrderBottom;
        candidates.retain(|(oid, _)| !chosen.contains(oid));
        self.state.pending_resolution = Some(pending);
        Ok(finish_with_events(self, ev))
    }
}

#[cfg(test)]
mod into_the_wilds_tests {
    use super::*;

    #[test]
    fn deploy_private_fixture_preserves_previous_result_and_tail_once_across_all_entry_choices() {
        for count in 0..=2 {
            let mut engine = GameEngine::new(90_210, &[0, 1], 20, None, true).unwrap();
            engine.state.turn_step = TurnStep::Main1;
            battlefield_card(&mut engine, "orb_of_dreams");
            battlefield_card(&mut engine, "orb_of_dreams");
            let first = library_card(&mut engine, "jace_beleren");
            let second = library_card(&mut engine, "chandra,_novice_pyromancer");
            let rest = library_card(&mut engine, "forest");
            let looked = [first, second, rest];
            engine.state.players[0]
                .library
                .retain(|oid| !looked.contains(oid));
            for &oid in looked.iter().rev() {
                engine.state.players[0].library.push_front(oid);
            }
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
            let tricerules_cards::primitives::ResolutionBranchRequirement::CardResultCount {
                filter,
                ..
            } = &mut branches[0].requirement
            else {
                unreachable!()
            };
            filter.action = tricerules_cards::primitives::CardResultAction::Mill;
            branches[0].effects = vec![SpellEffectKind::GainLife {
                amount: Amount::Fixed(3),
            }];
            let mut item = test_stack_item();
            item.card_id = "deploy_the_gatewatch".into();
            item.is_triggered = true;
            item.ability_text = Some("private Deploy continuation fixture".into());
            let mut ability = engine
                .registry
                .get("into_the_wilds")
                .unwrap()
                .primary_face()
                .triggered_abilities[0]
                .clone();
            ability.effect = vec![SpellEffectKind::DeployTheGatewatch, tail];
            item.triggered_ability = Some(ability);
            let mut stack = ParkedStackResolution::new(item);
            stack.resume_effect_index = Some(1);
            stack
                .previous_result
                .cards
                .push(crate::state::CardResultEntry {
                    action: tricerules_cards::primitives::CardResultAction::Mill,
                    affected_player: 0,
                    object_id: rest,
                    zone_change_generation: 0,
                    matched_card_types: vec![CardTypeFilter::BasicLand],
                });
            engine.state.pending_resolution = Some(PendingResolution {
                deciding_player: 0,
                presentation: PendingResolutionPresentation {
                    source_object_id: 90_001,
                    candidates: vec![first, second],
                    min: 0,
                    max: 2,
                    ordered: false,
                    unique_names: false,
                    prompt: "fixture".into(),
                    choice_kind: custom::ChoiceKind::LibraryLook,
                },
                continuation: ResolutionContinuation::LibraryLook {
                    stack,
                    stage: PendingLibraryLookStage::DeployTheGatewatch,
                    candidates: looked.iter().map(|&oid| (oid, 0)).collect(),
                },
            });
            let selected = [first, second][..count].to_vec();
            let mut batches = vec![engine
                .submit_resolution_choice(
                    0,
                    &rv1::SubmitResolutionChoice {
                        chosen_object_ids: selected.clone(),
                        ..Default::default()
                    },
                )
                .unwrap()];
            for _ in 0..10 {
                let Some(pending) = engine.state.pending_resolution.as_ref() else {
                    break;
                };
                assert_eq!(
                    engine.state.players[0].life, 20,
                    "tail waits for entry and bottom completion"
                );
                assert!(looked
                    .iter()
                    .all(|oid| engine.state.objects[oid].zone == Zone::Library));
                let chosen =
                    if pending.presentation.choice_kind == custom::ChoiceKind::ReplacementEffect {
                        vec![pending.presentation.candidates[0]]
                    } else {
                        assert_eq!(
                            pending.presentation.choice_kind,
                            custom::ChoiceKind::SimultaneousEntryOrder
                        );
                        pending.presentation.candidates.clone()
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
            assert_eq!(engine.state.players[0].life, 23);
            for oid in selected {
                assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
                assert!(engine.state.objects[&oid].tapped);
            }
            let bottom_count = 3 - count;
            assert!(
                engine.state.players[0]
                    .library
                    .iter()
                    .rev()
                    .take(bottom_count)
                    .any(|oid| *oid == rest),
                "the remainder is bottomed before the saved tail finishes"
            );
            assert_eq!(batches.iter().flat_map(|batch| &batch.events).filter(|event| matches!(&event.ev, Some(rv1::ruled_event::Ev::Log(log)) if log.text.contains("in a random order"))).count(), 1);
            assert!(engine.state.stack.is_empty());
        }
    }
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
    fn optional_land_entry_preserves_saved_previous_result_and_runs_tail_once() {
        for accept in [false, true] {
            let mut engine = GameEngine::new(90_200, &[0, 1], 20, None, true).unwrap();
            engine.state.turn_step = TurnStep::Upkeep;
            battlefield_card(&mut engine, "orb_of_dreams");
            battlefield_card(&mut engine, "orb_of_dreams");
            let forest = library_card(&mut engine, "forest");
            engine.state.players[0].library.retain(|id| *id != forest);
            engine.state.players[0].library.push_front(forest);
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
            let tricerules_cards::primitives::ResolutionBranchRequirement::CardResultCount {
                filter,
                ..
            } = &mut branches[0].requirement
            else {
                unreachable!()
            };
            filter.action = tricerules_cards::primitives::CardResultAction::Mill;
            branches[0].effects = vec![SpellEffectKind::GainLife {
                amount: Amount::Fixed(3),
            }];
            let mut item = test_stack_item();
            item.card_id = "into_the_wilds".into();
            item.is_triggered = true;
            item.ability_text = Some("private look plus sentinel".into());
            let mut ability = engine
                .registry
                .get("into_the_wilds")
                .unwrap()
                .primary_face()
                .triggered_abilities[0]
                .clone();
            ability.effect = vec![SpellEffectKind::IntoTheWilds, tail];
            item.triggered_ability = Some(ability);
            let mut stack = ParkedStackResolution::new(item);
            stack.resume_effect_index = Some(1);
            stack
                .previous_result
                .cards
                .push(crate::state::CardResultEntry {
                    action: tricerules_cards::primitives::CardResultAction::Mill,
                    affected_player: 0,
                    object_id: forest,
                    zone_change_generation: 0,
                    matched_card_types: vec![CardTypeFilter::BasicLand],
                });
            engine.state.pending_resolution = Some(PendingResolution {
                deciding_player: 0,
                presentation: PendingResolutionPresentation {
                    source_object_id: 90_001,
                    candidates: vec![forest],
                    min: 0,
                    max: 1,
                    ordered: false,
                    unique_names: false,
                    prompt: "look".into(),
                    choice_kind: custom::ChoiceKind::LibraryLook,
                },
                continuation: ResolutionContinuation::LibraryLook {
                    stack,
                    stage: PendingLibraryLookStage::IntoTheWilds,
                    candidates: vec![(forest, 0)],
                },
            });
            let selected = if accept { vec![forest] } else { Vec::new() };
            let mut batches = vec![engine
                .submit_resolution_choice(
                    0,
                    &rv1::SubmitResolutionChoice {
                        chosen_object_ids: selected,
                        ..Default::default()
                    },
                )
                .unwrap()];
            if accept {
                let pending = engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .expect("entry replacement pauses");
                assert_eq!(
                    pending.presentation.choice_kind,
                    custom::ChoiceKind::ReplacementEffect
                );
                assert_eq!(
                    engine.state.players[0].life, 20,
                    "tail waits for accepted entry"
                );
                assert_eq!(engine.state.objects[&forest].zone, Zone::Library);
                let chosen = pending.presentation.candidates[0];
                batches.push(
                    engine
                        .submit_resolution_choice(
                            0,
                            &rv1::SubmitResolutionChoice {
                                chosen_object_ids: vec![chosen],
                                ..Default::default()
                            },
                        )
                        .unwrap(),
                );
                assert_eq!(engine.state.objects[&forest].zone, Zone::Battlefield);
                assert!(engine.state.objects[&forest].tapped);
                assert_eq!(engine.state.zone_change_generation[&forest], 1);
            } else {
                assert_eq!(engine.state.players[0].library.front(), Some(&forest));
                assert_eq!(
                    engine
                        .state
                        .zone_change_generation
                        .get(&forest)
                        .copied()
                        .unwrap_or(0),
                    0
                );
            }
            assert!(engine.state.pending_resolution.is_none());
            assert_eq!(
                engine.state.players[0].life, 23,
                "saved result admits tail exactly once"
            );
            assert!(!batches.iter().flat_map(|batch| &batch.events).any(|event|
                matches!(&event.ev, Some(rv1::ruled_event::Ev::Log(log)) if log.text.contains("shuffles"))));
            assert!(engine.state.stack.is_empty());
        }
    }
}
