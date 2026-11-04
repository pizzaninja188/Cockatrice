//! As Foretold changes a cast's base cost; it grants neither timing nor zone permission.
use super::*;

impl GameEngine {
    pub(super) fn as_foretold_cost_options(
        &self,
        player: PlayerId,
        face: &CardFace,
    ) -> Vec<rv1::AsForetoldCostOption> {
        if face.is_land
            || self
                .state
                .player_idx(player)
                .is_none_or(|i| self.state.players[i].has_lost)
        {
            return Vec::new();
        }
        let mut sources = self.state.objects.keys().copied().collect::<Vec<_>>();
        sources.sort_unstable();
        sources
            .into_iter()
            .filter_map(|id| {
                let object = &self.state.objects[&id];
                let generation = self
                    .state
                    .zone_change_generation
                    .get(&id)
                    .copied()
                    .unwrap_or(0);
                if object.zone != Zone::Battlefield
                    || self.controller_of(id) != Some(player)
                    || object.counter_count(CounterKind::Time) < face.mana_cost.mana_value()
                    || self.state.as_foretold_uses.contains(&(
                        self.state.turn_instance,
                        id,
                        generation,
                    ))
                    || !self
                        .active_static_ability_definitions(id)
                        .contains(&StaticAbilityDef::AsForetoldAlternativeCost)
                {
                    return None;
                }
                Some(rv1::AsForetoldCostOption {
                    source: Some(rv1::CostObjectRef {
                        object_id: id,
                        zone_change_generation: generation,
                    }),
                    label: format!(
                        "As Foretold — {} time counters",
                        object.counter_count(CounterKind::Time)
                    ),
                    cost: "{0}".into(),
                })
            })
            .collect()
    }

    /// Capture before costs. Successful completion consumes this exact receipt without looking
    /// up the source again: paying a cost may remove it or change its counters/controller.
    pub(super) fn prepare_as_foretold_cost(
        &self,
        player: PlayerId,
        command: &rv1::CastSpell,
        face: &CardFace,
        method: SpellCastMethod,
    ) -> Result<Option<(u64, ObjectId, u64)>, EngineError> {
        let Some(selected) = command.as_foretold_source else {
            return Ok(None);
        };
        if method != SpellCastMethod::Normal || command.x_value != 0 {
            return Err(EngineError::Illegal(
                "As Foretold cannot combine with another alternative cost or nonzero X",
            ));
        }
        if !self
            .as_foretold_cost_options(player, face)
            .iter()
            .any(|option| option.source == Some(selected))
        {
            return Err(EngineError::Illegal(
                "missing, stale or unavailable As Foretold cost source",
            ));
        }
        Ok(Some((
            self.state.turn_instance,
            selected.object_id,
            selected.zone_change_generation,
        )))
    }
}
