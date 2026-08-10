use super::*;

impl GameEngine {
    /// Record a committed simultaneous event set. This is deliberately separate from trigger
    /// matching: transactional cast/activation checks may collect prospective triggers, but turn
    /// history must only observe changes that actually reached game state.
    pub(super) fn record_committed_events(&mut self, events: &[GameEvent]) {
        let deaths = events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    GameEvent::Dies {
                        was_creature: true,
                        ..
                    }
                )
            })
            .count() as u32;
        self.state.turn_history.current.creatures_died = self
            .state
            .turn_history
            .current
            .creatures_died
            .saturating_add(deaths);
    }

    pub(super) fn record_spell_cast(&mut self) {
        self.state.turn_history.current.spells_cast = self
            .state
            .turn_history
            .current
            .spells_cast
            .saturating_add(1);
    }

    pub(super) fn condition_holds(&self, condition: GameCondition) -> bool {
        match condition {
            GameCondition::CreatureDeathsThisTurn { .. } => {
                condition.matches_count(self.state.turn_history.current.creatures_died)
            }
        }
    }

    pub(super) fn resolve_amount(&self, amount: Amount, chosen_x: u32) -> u32 {
        match amount {
            Amount::Fixed(value) => value,
            Amount::X => chosen_x,
            Amount::Conditional {
                condition,
                when_true,
                otherwise,
            } => {
                if self.condition_holds(condition) {
                    when_true
                } else {
                    otherwise
                }
            }
        }
    }
}
