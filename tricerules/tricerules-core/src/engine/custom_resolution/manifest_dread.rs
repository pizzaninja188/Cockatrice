use super::*;
use crate::engine::resolution::permanent_moved_event_with_library_position;

impl GameEngine {
    pub(super) fn finish_manifest_dread(
        &mut self,
        pending: PendingResolution,
        chosen: ObjectId,
    ) -> Result<RuledEventBatch, EngineError> {
        let controller = pending.deciding_player;
        let chosen_position = pending
            .scratch
            .iter()
            .position(|object_id| *object_id == chosen)
            .ok_or(EngineError::Illegal("manifest choice is stale"))?
            as u32;
        let other = pending
            .scratch
            .iter()
            .copied()
            .find(|object_id| *object_id != chosen);
        let current_top: Vec<ObjectId> = self
            .state
            .player_idx(controller)
            .map(|idx| {
                self.state.players[idx]
                    .library
                    .iter()
                    .take(2)
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
        if current_top != pending.scratch {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("manifest choice is stale"));
        }
        self.state
            .objects
            .get_mut(&chosen)
            .ok_or(EngineError::Illegal("manifest card is missing"))?
            .face_down = true;

        let mut events = Vec::new();
        let entry = BattlefieldEntryEvent {
            object_id: chosen,
            deciding_player: controller,
            destination_controller: controller,
            face_index: 0,
            chosen_x: 0,
            player_life_snapshot: self.player_life_snapshot(),
            tapped: false,
            entry_counters: BTreeMap::new(),
            applied_effects: Vec::new(),
        };
        let completion = BattlefieldEntryCompletion::ManifestDread {
            owner: controller,
            other_object_id: other,
            chosen_library_position: chosen_position,
        };
        match self.begin_battlefield_entry(pending.item.clone(), entry, completion, &mut events) {
            super::super::replacement::BattlefieldEntryProgress::Parked => {
                if let Some(replacement_pending) = self.state.pending_resolution.as_mut() {
                    replacement_pending.resume_effect_index = pending.resume_effect_index;
                }
                Ok(finish_with_events(self, events))
            }
            super::super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                self.commit_battlefield_entry(entry, None)?;
                events.push(permanent_moved_event_with_library_position(
                    &self.state,
                    chosen,
                    controller,
                    rv1::permanent_moved::Destination::Battlefield,
                    chosen_position,
                ));
                if let Some(other) = other {
                    move_object_to_zone(
                        &mut self.state,
                        self.registry,
                        other,
                        Zone::Graveyard,
                        None,
                    )?;
                    events.push(permanent_moved_event_with_library_position(
                        &self.state,
                        other,
                        controller,
                        rv1::permanent_moved::Destination::Graveyard,
                        0,
                    ));
                }
                events.push(ev_log(format!("P{controller} manifests dread.")));
                self.complete_parked_resolution(pending.item, pending.resume_effect_index, events)
            }
        }
    }
}
