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
            &mut ev,
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
}
