use super::*;

impl GameEngine {
    /// Resolve a caster-chooses DiscardCards interrupt: move chosen cards from the target's hand
    /// to their graveyard, then restore priority to the active player.
    pub(super) fn finish_discard_chosen(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let discard = pending
            .discard
            .ok_or(EngineError::Illegal("discard continuation missing"))?;
        let card_name = self
            .registry
            .get(&pending.item.card_id)
            .and_then(|d| d.face(pending.item.face_index))
            .map(|f| f.name.to_string())
            .unwrap_or_else(|| pending.item.card_id.clone());

        let mut ev = vec![];
        for &oid in chosen {
            let owner = self
                .state
                .objects
                .get(&oid)
                .map(|o| o.owner)
                .ok_or(EngineError::Illegal("chosen card object not found"))?;
            if owner != discard.affected_player {
                return Err(EngineError::Illegal(
                    "chosen card is not owned by the affected player",
                ));
            }
            let discard_name = object_display_name(&self.state, self.registry, oid);
            move_object_to_zone(&mut self.state, self.registry, oid, Zone::Graveyard, None)?;
            ev.push(permanent_moved_event(
                &self.state,
                oid,
                owner,
                rv1::permanent_moved::Destination::Graveyard,
            ));
            ev.push(ev_log(format!(
                "P{owner} discards {discard_name} ({card_name})."
            )));
        }
        if discard.draw_after > 0 && (!discard.draw_only_if_discarded || !chosen.is_empty()) {
            resolution::zones::draw_cards_for_player(
                self,
                &mut ev,
                discard.affected_player,
                discard.draw_after,
                &card_name,
            )?;
        }
        self.complete_parked_resolution(pending.item, pending.resume_effect_index, ev)
    }
}
