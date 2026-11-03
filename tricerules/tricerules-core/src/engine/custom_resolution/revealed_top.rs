//! The optional Bottom branch for one unmatched publicly revealed top card.
use super::*;

impl GameEngine {
    fn revealed_top_is_current(
        &self,
        owner: PlayerId,
        object_id: ObjectId,
        generation: u64,
    ) -> bool {
        self.state.player_idx(owner).is_some_and(|index| {
            !self.state.players[index].has_lost
                && self.state.players[index].library.front() == Some(&object_id)
                && self
                    .state
                    .objects
                    .get(&object_id)
                    .is_some_and(|object| object.zone == Zone::Library && object.owner == owner)
                && self
                    .state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0)
                    == generation
        })
    }

    pub(in crate::engine) fn revealed_top_bottom_choice_event(&self) -> Option<rv1::RuledEvent> {
        let pending = self.state.pending_resolution.as_ref()?;
        let ResolutionContinuation::RevealedTopBottom { public_reveal, .. } = &pending.continuation
        else {
            return None;
        };
        Some(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: pending.presentation.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                    min: 1,
                    max: 1,
                    public_reveal: Some(public_reveal.clone()),
                    resolution_branches: ["Keep on top", "Bottom"]
                        .into_iter()
                        .enumerate()
                        .map(|(index, label)| rv1::ResolutionBranchOption {
                            branch_index: index as u32,
                            label: label.into(),
                            selectable: true,
                            ..Default::default()
                        })
                        .collect(),
                    ..Default::default()
                },
            )),
        })
    }

    pub(super) fn finish_revealed_top_bottom(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, owner, object_id, generation) = match &pending.continuation {
            ResolutionContinuation::RevealedTopBottom {
                stack,
                library_owner,
                object_id,
                zone_change_generation,
                ..
            } => (
                stack.clone(),
                *library_owner,
                *object_id,
                *zone_change_generation,
            ),
            _ => return Err(EngineError::Illegal("revealed top continuation missing")),
        };
        if decision != rv1::ResolutionChoiceDecision::SelectBranch
            || answer.selected_branch_index > 1
            || !answer.chosen_object_ids.is_empty()
            || !answer.chosen_player_ids.is_empty()
            || answer.payment.is_some()
            || !answer.restricted_mana.is_empty()
            || answer.cast_spell.is_some()
            || answer.spell_cast_announcement.is_some()
            || answer.chosen_combat_defender.is_some()
            || pending.deciding_player != owner
            || !self.revealed_top_is_current(owner, object_id, generation)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "revealed top choice became stale or malformed",
            ));
        }
        let mut events = Vec::new();
        if answer.selected_branch_index == 1 {
            // The shared helper filters stale refs. Validate exact TOP above before any mutation.
            self.bottom_revealed_library_refs(
                owner,
                &[(object_id, generation)],
                None,
                &mut events,
            )?;
        } else {
            events.push(ev_log(format!(
                "P{owner} keeps the revealed card on top of their library."
            )));
        }
        let previous = stack.previous_result().clone();
        self.complete_parked_stack_with_previous(stack, previous, events)
    }

    pub(in crate::engine) fn refresh_revealed_top_bottom_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let (stack, owner, object_id, generation) = match &pending.continuation {
            ResolutionContinuation::RevealedTopBottom {
                stack,
                library_owner,
                object_id,
                zone_change_generation,
                ..
            } => (
                stack.clone(),
                *library_owner,
                *object_id,
                *zone_change_generation,
            ),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
        };
        if self.revealed_top_is_current(owner, object_id, generation) {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }
        let previous = stack.previous_result().clone();
        let completed = self.complete_parked_stack_with_previous(stack, previous, Vec::new())?;
        events.extend(completed.events);
        Ok(())
    }
}
