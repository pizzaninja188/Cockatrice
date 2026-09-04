use super::*;

impl GameEngine {
    pub(super) fn finish_explore_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, explorer, revealed) = match &pending.continuation {
            ResolutionContinuation::Explore {
                stack,
                explorer,
                revealed,
            } => (stack.clone(), *explorer, *revealed),
            _ => return Err(EngineError::Illegal("Explore continuation missing")),
        };
        let controller = pending.deciding_player;
        let current_generation = self
            .state
            .zone_change_generation
            .get(&revealed.object_id)
            .copied()
            .unwrap_or(0);
        let revealed_is_current = current_generation == revealed.zone_change_generation
            && self
                .state
                .objects
                .get(&revealed.object_id)
                .is_some_and(|object| object.zone == Zone::Library)
            && self
                .state
                .player_idx(controller)
                .and_then(|idx| self.state.players[idx].library.front().copied())
                == Some(revealed.object_id);
        if !revealed_is_current {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "Explore revealed card is no longer on top of the library",
            ));
        }

        let mut events = Vec::new();
        let revealed_name = object_display_name(&self.state, self.registry, revealed.object_id);
        if chosen.is_empty() {
            events.push(ev_log(format!(
                "P{controller} leaves {revealed_name} on top of their library."
            )));
        } else {
            let owner = self
                .state
                .objects
                .get(&revealed.object_id)
                .map(|object| object.owner)
                .ok_or(EngineError::Illegal("Explore revealed card missing"))?;
            move_object_to_zone(
                &mut self.state,
                self.registry,
                revealed.object_id,
                Zone::Graveyard,
                None,
            )?;
            events.push(permanent_moved_event_with_library_position(
                &self.state,
                revealed.object_id,
                owner,
                rv1::permanent_moved::Destination::Graveyard,
                0,
            ));
            events.push(ev_log(format!(
                "P{controller} puts {revealed_name} into their graveyard."
            )));
        }
        self.fire_triggers(&[GameEvent::Explored { object: explorer }]);
        self.complete_parked_resolution(stack.item, stack.resume_effect_index, events)
    }
}
