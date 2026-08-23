use super::*;

impl GameEngine {
    pub(super) fn finish_ward_discard(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, ward, candidate_generations) = match &pending.continuation {
            ResolutionContinuation::WardPayment {
                stack,
                ward:
                    ward @ PendingWardPayment {
                        stage:
                            PendingWardPaymentStage::Discard {
                                candidate_generations,
                            },
                        ..
                    },
            } => (stack.clone(), ward.clone(), candidate_generations.clone()),
            _ => return Err(EngineError::Illegal("Ward discard continuation missing")),
        };
        if chosen.len() > 1 {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "Ward discard requires at most one card",
            ));
        }

        let mut events = Vec::new();
        if let Some(&card) = chosen.first() {
            let expected_generation = candidate_generations
                .iter()
                .find_map(|(candidate, generation)| (*candidate == card).then_some(*generation));
            let current_generation = self
                .state
                .zone_change_generation
                .get(&card)
                .copied()
                .unwrap_or(0);
            let in_payer_hand = self
                .state
                .player_idx(pending.deciding_player)
                .is_some_and(|index| self.state.players[index].hand.contains(&card));
            if expected_generation != Some(current_generation) || !in_payer_hand {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("Ward discard choice became stale"));
            }
            let card_name = object_display_name(&self.state, self.registry, card);
            move_object_to_zone(&mut self.state, self.registry, card, Zone::Graveyard, None)?;
            events.push(permanent_moved_event(
                &self.state,
                card,
                pending.deciding_player,
                rv1::permanent_moved::Destination::Graveyard,
            ));
            events.push(ev_log(format!(
                "P{} discards {card_name} to pay Ward.",
                pending.deciding_player
            )));
        } else {
            let label = stack
                .item
                .ability_text
                .as_deref()
                .unwrap_or("Ward")
                .to_string();
            counter_stack_object_ref(self, ward.target, &label, &mut events)?;
            events.push(ev_log(format!(
                "P{} declines to pay Ward.",
                pending.deciding_player
            )));
        }

        self.complete_parked_resolution(stack.item, stack.resume_effect_index, events)
    }
}
