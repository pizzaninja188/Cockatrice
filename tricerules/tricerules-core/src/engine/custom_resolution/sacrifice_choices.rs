use super::*;

impl GameEngine {
    /// CR 701.17: the target player has chosen which permanent to sacrifice.
    pub(super) fn finish_sacrifice_chosen(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let stack = match &pending.continuation {
            ResolutionContinuation::Sacrifice { stack } => stack.clone(),
            _ => return Err(EngineError::Illegal("sacrifice continuation missing")),
        };
        let oid = chosen[0];
        let card_name = super::events::object_display_name(&self.state, self.registry, oid);
        // Capture last-known information before the zone move clears transient state.
        let owner = self
            .state
            .objects
            .get(&oid)
            .map(|o| o.owner)
            .ok_or(EngineError::Illegal("sacrificed object missing"))?;
        let source = self
            .trigger_source_snapshot(oid)
            .ok_or(EngineError::Illegal("sacrificed object missing"))?;
        let was_creature = self
            .characteristics(oid)
            .is_some_and(|value| value.is_creature());

        let zone_snapshot = self.snapshot_zone_event();
        let died = sacrifice_permanent(&mut self.state, self.registry, oid)?;

        let mut ev = vec![
            permanent_moved_event(
                &self.state,
                oid,
                owner,
                rv1::permanent_moved::Destination::Graveyard,
            ),
            ev_log(format!(
                "P{} sacrifices {card_name}.",
                pending.deciding_player
            )),
        ];

        self.fire_zone_triggers(
            zone_snapshot,
            sacrifice_events(source, was_creature, pending.deciding_player, died),
        );
        let _ = self.apply_sbas(&mut ev);
        let result = CardResultCohort {
            cards: vec![payment::card_result_entry(
                &self.state,
                self.registry,
                CardResultAction::Sacrifice,
                pending.deciding_player,
                oid,
            )],
        };

        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            result.into(),
            ev,
        )
    }

    /// CR 704.5j: the controller has chosen which legend to keep. Put all other candidates into
    /// their owners' graveyards, emit their LTB / death events, then re-run SBAs.
    pub(super) fn finish_legend_sba_choice(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let keep_id = chosen[0];
        let mut ev = vec![];
        let mut trigger_events = vec![];
        let zone_snapshot = self.snapshot_zone_event();
        for &oid in &pending.presentation.candidates {
            if oid == keep_id {
                continue;
            }
            let owner = self.state.objects.get(&oid).map(|o| o.owner);
            let source = zone_snapshot.source(oid);
            let was_creature = self
                .characteristics(oid)
                .is_some_and(|value| value.is_creature());
            if let Ok(died) = put_permanent_in_graveyard(&mut self.state, self.registry, oid) {
                if let Some(owner_id) = owner {
                    ev.push(permanent_moved_event(
                        &self.state,
                        oid,
                        owner_id,
                        rv1::permanent_moved::Destination::Graveyard,
                    ));
                }
                if let Some(source) = source {
                    trigger_events.extend(leaves_and_dies_events(source, was_creature, died));
                }
            }
        }
        self.fire_zone_triggers(zone_snapshot, trigger_events);
        // Re-run SBAs: triggered abilities may have caused further state changes, and
        // multiple legend conflicts are resolved one at a time.
        if self.state.pending_resolution.is_none() {
            if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
                self.state.priority_idx = i;
            }
            self.apply_sbas(&mut ev)?;
            if self.state.pending_resolution.is_none() {
                ev.push(ev_priority_changed(self));
            }
        }
        Ok(finish_with_events(self, ev))
    }
}
