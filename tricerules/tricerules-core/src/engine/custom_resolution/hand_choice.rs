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
        for &object_id in chosen {
            resolution::zones::perform_hand_card_action(
                self,
                &mut events,
                hand_choice.affected_player,
                object_id,
                hand_choice.action,
                &card_name,
            )?;
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
        self.complete_parked_resolution(stack.item, stack.resume_effect_index, events)
    }
}
