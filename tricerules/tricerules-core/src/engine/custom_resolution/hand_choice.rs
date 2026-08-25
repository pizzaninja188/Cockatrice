use super::*;

impl GameEngine {
    /// Resolve a chosen hand-card action after revalidating the complete cohort. Discard and
    /// direct exile deliberately share only this selection boundary; their semantic mutation
    /// paths stay distinct for future discard triggers and replacement effects.
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

        let card_name = self
            .registry
            .get(&stack.item.card_id)
            .and_then(|definition| definition.face(stack.item.face_index))
            .map(|face| face.name.to_string())
            .unwrap_or_else(|| stack.item.card_id.clone());
        let mut events = vec![];
        let mut result = CardResultCohort::default();
        for &object_id in chosen {
            result
                .cards
                .push(resolution::zones::perform_hand_card_action(
                    self,
                    &mut events,
                    hand_choice.affected_player,
                    object_id,
                    hand_choice.action,
                    &card_name,
                )?);
        }
        if hand_choice.draw_after > 0 && (!hand_choice.draw_only_if_discarded || !chosen.is_empty())
        {
            resolution::zones::draw_cards_for_player(
                self,
                &mut events,
                hand_choice.affected_player,
                hand_choice.draw_after,
                &card_name,
            )?;
        }
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            result,
            events,
        )
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

        discard.selections.push(chosen.to_vec());
        if discard.current + 1 < discard.choices.len() {
            discard.current += 1;
            let mut events = Vec::new();
            resolution::zones::park_player_set_discard_choice(self, &mut events, stack, discard);
            return Ok(finish_with_events(self, events));
        }

        let all_current =
            discard
                .choices
                .iter()
                .zip(&discard.selections)
                .all(|(choice, selection)| {
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
                });
        if !all_current {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "staged discard card is stale or no longer legal",
            ));
        }

        let card_name = self
            .registry
            .get(&stack.item.card_id)
            .and_then(|definition| definition.face(stack.item.face_index))
            .map(|face| face.name.to_string())
            .unwrap_or_else(|| stack.item.card_id.clone());
        let mut events = Vec::new();
        let mut result = CardResultCohort::default();
        for (choice, selection) in discard.choices.iter().zip(&discard.selections) {
            for object_id in selection {
                result
                    .cards
                    .push(resolution::zones::perform_hand_card_action(
                        self,
                        &mut events,
                        choice.player,
                        *object_id,
                        HandCardAction::Discard,
                        &card_name,
                    )?);
            }
        }
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            result,
            events,
        )
    }
}
