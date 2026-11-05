use super::*;

impl GameEngine {
    pub(super) fn commander_tax(&self, player: PlayerId, object: ObjectId) -> u32 {
        self.state
            .player_idx(player)
            .and_then(|index| self.state.players[index].commander_cast_counts.get(&object))
            .copied()
            .unwrap_or(0)
            .saturating_mul(2)
    }

    pub(super) fn commander_cost(
        &self,
        player: PlayerId,
        object: ObjectId,
        mut cost: ManaCost,
    ) -> ManaCost {
        let tax = self.commander_tax(player, object);
        if tax > 0 {
            if let Some(ManaSymbol::Generic(amount)) = cost
                .pips
                .iter_mut()
                .find(|pip| matches!(pip, ManaSymbol::Generic(_)))
            {
                *amount = amount.saturating_add(tax);
            } else {
                cost.pips.insert(0, ManaSymbol::Generic(tax));
            }
        }
        cost
    }

    pub(super) fn record_commander_damage(&mut self, event: &damage::DamageEvent, dealt: u32) {
        use damage::{DamageClassification, DamageRecipient};
        let DamageRecipient::Player(player) = event.recipient else {
            return;
        };
        if event.classification != DamageClassification::Combat
            || dealt == 0
            || !self.state.players.iter().any(|owner| {
                owner
                    .declared_commander_object_ids
                    .contains(&event.source.object_id)
            })
        {
            return;
        }
        if let Some(index) = self.state.player_idx(player) {
            let total = self.state.players[index]
                .commander_damage
                .entry(event.source.object_id)
                .or_default();
            *total = total.saturating_add(dealt);
        }
    }
}
