use super::*;

impl GameEngine {
    /// Resolve a chosen hand-card action after revalidating the complete cohort. Discard and
    /// direct exile deliberately share only this selection boundary; their semantic mutation
    /// paths stay distinct so discard triggers and replacement effects observe only true discards.
    pub(super) fn finish_hand_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, hand_choice) = match &pending.continuation {
            ResolutionContinuation::HandChoice { stack, hand_choice } => {
                (stack.clone(), hand_choice.clone())
            }
            _ => return Err(EngineError::Illegal("hand-choice continuation missing")),
        };
        let Some(player_index) = self.state.player_idx(hand_choice.affected_player) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "affected hand player no longer exists",
            ));
        };
        let hand = &self.state.players[player_index].hand;
        let valid = chosen.iter().all(|object_id| {
            let Some(object) = self.state.objects.get(object_id) else {
                return false;
            };
            let generation_matches =
                hand_choice
                    .candidate_generations
                    .iter()
                    .any(|(candidate, generation)| {
                        candidate == object_id
                            && self
                                .state
                                .zone_change_generation
                                .get(object_id)
                                .copied()
                                .unwrap_or(0)
                                == *generation
                    });
            object.owner == hand_choice.affected_player
                && object.zone == Zone::Hand
                && hand.contains(object_id)
                && generation_matches
        });
        if !valid {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "chosen hand card is stale or no longer legal",
            ));
        }

        if hand_choice.program_selection {
            let selected_hand_cards = chosen
                .iter()
                .map(|object_id| {
                    let generation = hand_choice
                        .candidate_generations
                        .iter()
                        .find_map(|(candidate, generation)| {
                            (candidate == object_id).then_some(*generation)
                        })
                        .expect("hand choice validated candidate generation");
                    TriggerObjectRef {
                        object_id: *object_id,
                        zone_change_generation: generation,
                        controller_at_event: hand_choice.affected_player,
                    }
                })
                .collect();
            return self.complete_parked_stack_with_previous(
                stack,
                EffectResult {
                    selected_hand_cards,
                    ..Default::default()
                },
                vec![ev_log(format!(
                    "P{} chooses {} card{}.",
                    hand_choice.affected_player,
                    chosen.len(),
                    if chosen.len() == 1 { "" } else { "s" },
                ))],
            );
        }

        if hand_choice.action == HandCardAction::Discard
            && !chosen.is_empty()
            && self.has_discard_library_replacement(hand_choice.affected_player)
        {
            return self.start_discard_replacements(
                stack,
                chosen
                    .iter()
                    .map(|oid| {
                        (
                            hand_choice.affected_player,
                            *oid,
                            crate::state::DiscardCause::Effect,
                        )
                    })
                    .collect(),
                hand_choice.revealed,
            );
        }
        let card_name = self
            .registry
            .get(&stack.item.card_id)
            .and_then(|definition| definition.face(stack.item.face_index))
            .map(|face| face.name.to_string())
            .unwrap_or_else(|| stack.item.card_id.clone());
        let mut events = vec![];
        if hand_choice.action == HandCardAction::PutOntoBattlefield {
            let Some(&object_id) = chosen.first() else {
                return self.complete_parked_stack_with_previous(
                    stack,
                    EffectResult::default(),
                    events,
                );
            };
            if chosen.len() != 1 {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "put-onto-battlefield hand choice selects at most one card",
                ));
            }
            let object = self
                .state
                .objects
                .get(&object_id)
                .expect("validated hand-choice object");
            let owner = object.owner;
            let object_label = object_display_name(&self.state, self.registry, object_id);
            let controller = hand_choice.affected_player;
            let completion = BattlefieldEntryCompletion::ResolutionEffect {
                owner,
                spell_label: card_name,
                object_label,
                from_zone: Zone::Hand,
                triggered_return_generation: None,
            };
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
                entry_counters: Default::default(),
                entry_counter_placements: Vec::new(),
                entry_counter_replacements_resolved: false,
                entry_modifiers: Vec::new(),
                entry_cause: hand_choice.entry_cause,
                attached_to: None,
                pending_copy_candidate: None,
                pending_aura_recipient: None,
                accepted_aura_recipient: None,
                applied_effects: Vec::new(),
            };
            return match self.begin_battlefield_entry(
                stack.item.clone(),
                entry,
                completion.clone(),
                &mut events,
            )? {
                super::super::replacement::BattlefieldEntryProgress::Parked => {
                    self.transfer_entry_choice_resume(&stack);
                    Ok(finish_with_events(self, events))
                }
                super::super::replacement::BattlefieldEntryProgress::Skipped(entry) => {
                    self.restore_skipped_battlefield_entry(&entry)?;
                    self.complete_parked_stack_with_previous(stack, EffectResult::default(), events)
                }
                super::super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                    self.complete_pending_battlefield_entry(pending, *entry, completion, events)
                }
            };
        }
        let mut result = CardResultCohort::default();
        let mut discard_receipts = Vec::new();
        for &object_id in chosen {
            let (entry, discard_receipt) = resolution::zones::perform_hand_card_action(
                self,
                &mut events,
                hand_choice.affected_player,
                object_id,
                hand_choice.action,
                &card_name,
            )?;
            result.cards.push(entry);
            discard_receipts.extend(discard_receipt);
        }
        if !discard_receipts.is_empty() {
            self.fire_discard_batches(
                vec![(hand_choice.affected_player, discard_receipts)],
                &mut events,
            );
        }
        self.complete_parked_stack_with_previous(stack, result.into(), events)
    }

    pub(super) fn finish_player_set_discard_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, mut discard) = match &pending.continuation {
            ResolutionContinuation::PlayerSetDiscard { stack, discard } => {
                (stack.clone(), discard.clone())
            }
            _ => {
                return Err(EngineError::Illegal(
                    "player-set discard continuation missing",
                ))
            }
        };
        let choice = discard
            .choices
            .get(discard.current)
            .expect("player-set discard current choice")
            .clone();
        let cause = choice.selection_cause(chosen);
        if cause.is_none()
            || (cause == Some(crate::state::DiscardCause::Cost)
                && !zone_card_matches_filter(
                    &self.state,
                    self.registry,
                    chosen[0],
                    choice.alternative_filter.as_ref(),
                ))
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "chosen cards do not satisfy a discard alternative",
            ));
        }
        let valid = chosen.iter().all(|object_id| {
            let Some(object) = self.state.objects.get(object_id) else {
                return false;
            };
            let generation_matches =
                choice
                    .candidate_generations
                    .iter()
                    .any(|(candidate, generation)| {
                        candidate == object_id
                            && self
                                .state
                                .zone_change_generation
                                .get(object_id)
                                .copied()
                                .unwrap_or(0)
                                == *generation
                    });
            let hand_contains = self
                .state
                .player_idx(choice.player)
                .is_some_and(|index| self.state.players[index].hand.contains(object_id));
            object.owner == choice.player
                && object.zone == Zone::Hand
                && hand_contains
                && generation_matches
        });
        if !valid {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "chosen hand card is stale or no longer legal",
            ));
        }

        let mut events = Vec::new();
        if choice.variable {
            events.push(ev_log(format!(
                "P{} chooses {} cards to discard.",
                choice.player,
                chosen.len()
            )));
        }
        discard.selections.push(chosen.to_vec());
        discard.current += 1;
        self.prune_departed_player_set_discard_rows(&mut discard)?;
        if discard.current < discard.choices.len() {
            resolution::zones::park_player_set_discard_choice(self, &mut events, stack, discard);
            return Ok(finish_with_events(self, events));
        }

        if !self.player_set_discard_selections_are_current(&discard) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "staged discard card is stale or no longer legal",
            ));
        }

        let Some(result) = self.apply_player_set_discard(stack.clone(), &discard, &mut events)?
        else {
            return Ok(finish_with_events(self, events));
        };
        self.complete_parked_stack_with_previous(stack, result.into(), events)
    }

    fn player_set_discard_selections_are_current(&self, discard: &PendingPlayerSetDiscard) -> bool {
        discard.choices.len() == discard.selections.len()
            && discard
                .choices
                .iter()
                .zip(&discard.selections)
                .all(|(choice, selection)| {
                    let cause = choice.selection_cause(selection);
                    if cause.is_none()
                        || (cause == Some(crate::state::DiscardCause::Cost)
                            && !zone_card_matches_filter(
                                &self.state,
                                self.registry,
                                selection[0],
                                choice.alternative_filter.as_ref(),
                            ))
                    {
                        return false;
                    }
                    selection.iter().all(|object_id| {
                        let Some(object) = self.state.objects.get(object_id) else {
                            return false;
                        };
                        let generation_matches =
                            choice
                                .candidate_generations
                                .iter()
                                .any(|(candidate, generation)| {
                                    candidate == object_id
                                        && self
                                            .state
                                            .zone_change_generation
                                            .get(object_id)
                                            .copied()
                                            .unwrap_or(0)
                                            == *generation
                                });
                        let hand_contains =
                            self.state.player_idx(choice.player).is_some_and(|index| {
                                self.state.players[index].hand.contains(object_id)
                            });
                        object.owner == choice.player
                            && object.zone == Zone::Hand
                            && hand_contains
                            && generation_matches
                    })
                })
    }

    fn prune_departed_player_set_discard_rows(
        &self,
        discard: &mut PendingPlayerSetDiscard,
    ) -> Result<bool, EngineError> {
        if discard.current > discard.choices.len() || discard.selections.len() != discard.current {
            return Err(EngineError::Illegal(
                "player-set discard accepted prefix is misaligned",
            ));
        }
        let original_current = discard.current;
        let mut choices = Vec::with_capacity(discard.choices.len());
        let mut selections = Vec::with_capacity(discard.selections.len());
        for (index, choice) in discard.choices.iter().enumerate() {
            let departed = self
                .state
                .player_idx(choice.player)
                .is_none_or(|player_index| self.state.players[player_index].has_lost);
            if departed {
                continue;
            }
            choices.push(choice.clone());
            if index < original_current {
                selections.push(discard.selections[index].clone());
            }
        }
        let changed = choices.len() != discard.choices.len();
        discard.choices = choices;
        discard.selections = selections;
        discard.current = discard.selections.len();
        Ok(changed)
    }

    /// Retire departed chooser rows without reopening the operation or its accepted live prefix.
    /// Restoration forces a new offer ID even when only the saved outer prompt was interrupted.
    pub(in crate::engine) fn reconcile_pending_player_set_discard_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
        force_reoffer: bool,
    ) -> Result<bool, EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(false);
        };
        let ResolutionContinuation::PlayerSetDiscard { stack, discard } = &pending.continuation
        else {
            return Ok(false);
        };
        let stack = stack.clone();
        let mut discard = discard.clone();
        let changed = self.prune_departed_player_set_discard_rows(&mut discard)?;
        if !changed && !force_reoffer {
            return Ok(false);
        }
        if discard.current < discard.choices.len() {
            resolution::zones::park_player_set_discard_choice(self, events, stack, discard);
        } else {
            if !self.player_set_discard_selections_are_current(&discard) {
                return Err(EngineError::Illegal(
                    "staged discard card is stale or no longer legal",
                ));
            }
            self.state.pending_resolution = None;
            let Some(result) = self.apply_player_set_discard(stack.clone(), &discard, events)?
            else {
                return Ok(true);
            };
            let completed =
                self.complete_parked_stack_with_previous(stack, result.into(), Vec::new())?;
            events.extend(completed.events);
        }
        Ok(true)
    }

    /// Commit a fully selected cohort, or park its discard replacements without finishing the tail.
    pub(in crate::engine) fn apply_player_set_discard(
        &mut self,
        mut stack: ParkedStackResolution,
        discard: &PendingPlayerSetDiscard,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<CardResultCohort>, EngineError> {
        let program_discard_outcome = stack.program.as_ref().map(|_| {
            discard
                .choices
                .iter()
                .enumerate()
                .find(|(_, choice)| choice.optional_exact)
                .map(|(index, _)| {
                    if discard.selections.get(index).is_some_and(Vec::is_empty) {
                        tricerules_card_model::DiscardResultOutcome::Declined
                    } else {
                        tricerules_card_model::DiscardResultOutcome::ChosenToPay
                    }
                })
                .unwrap_or(tricerules_card_model::DiscardResultOutcome::Performed)
        });
        if discard
            .choices
            .iter()
            .zip(&discard.selections)
            .any(|(choice, selection)| {
                !selection.is_empty()
                    && choice.selection_cause(selection) == Some(crate::state::DiscardCause::Effect)
                    && self.has_discard_library_replacement(choice.player)
            })
        {
            if let Some(outcome) = program_discard_outcome {
                stack.previous_result.program_discard_result =
                    Some(crate::state::ProgramDiscardResult {
                        outcome,
                        batches: Vec::new(),
                        cards: Vec::new(),
                    });
            }
            let selected = discard
                .choices
                .iter()
                .zip(&discard.selections)
                .flat_map(|(choice, selection)| {
                    selection.iter().map(|oid| {
                        (
                            choice.player,
                            *oid,
                            choice
                                .selection_cause(selection)
                                .expect("validated discard selection"),
                        )
                    })
                })
                .collect();
            let batch = self.start_discard_replacements(stack, selected, false)?;
            events.extend(batch.events);
            return Ok(None);
        }
        let card_name = self
            .registry
            .get(&stack.item.card_id)
            .and_then(|definition| definition.face(stack.item.face_index))
            .map(|face| face.name.to_string())
            .unwrap_or_else(|| stack.item.card_id.clone());
        let mut result = CardResultCohort::default();
        let mut discard_receipts: Vec<(PlayerId, Vec<crate::state::DiscardReceipt>)> = Vec::new();
        let mut typed_cards = Vec::new();
        for (choice, selection) in discard.choices.iter().zip(&discard.selections) {
            for object_id in selection {
                let (entry, discard_receipt) = resolution::zones::perform_discard_action(
                    self,
                    events,
                    choice.player,
                    *object_id,
                    &card_name,
                    choice
                        .selection_cause(selection)
                        .expect("validated discard selection"),
                )?;
                typed_cards.push(crate::state::DiscardCardResultEntry {
                    affected_player: choice.player,
                    type_knowledge: if discard_receipt.destination == Zone::Library
                        && discard_receipt.known_card_id.is_none()
                    {
                        crate::state::DiscardCardTypeKnowledge::UnknownHidden
                    } else {
                        crate::state::DiscardCardTypeKnowledge::Known(
                            entry.matched_card_types.clone(),
                        )
                    },
                });
                result.cards.push(entry);
                match discard_receipts
                    .iter_mut()
                    .find(|(player, _)| *player == choice.player)
                {
                    Some((_, receipts)) => receipts.push(discard_receipt),
                    None => discard_receipts.push((choice.player, vec![discard_receipt])),
                }
            }
        }
        let batches = self.fire_discard_batches(discard_receipts, events);
        if let Some(outcome) = program_discard_outcome {
            result.program_discard_result = Some(crate::state::ProgramDiscardResult {
                outcome,
                batches,
                cards: typed_cards,
            });
        }
        Ok(Some(result))
    }
}
