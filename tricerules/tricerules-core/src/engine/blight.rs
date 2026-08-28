//! Shared CR 701.68 operation for Cinder Strike, Gristle Glutton and Chaos Spewer.
use super::*;
use crate::state::BlightReceipt;

impl GameEngine {
    pub(super) fn blight_candidates(&self, player: PlayerId) -> Vec<ObjectId> {
        let mut candidates = self
            .state
            .objects
            .keys()
            .copied()
            .filter(|&oid| self.can_blight_creature(player, oid))
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        candidates
    }

    pub(super) fn can_blight_creature(&self, player: PlayerId, oid: ObjectId) -> bool {
        self.can_receive_counters(oid)
            && self
                .characteristics(oid)
                .is_some_and(|c| c.controller == player && c.is_creature())
    }

    pub(super) fn validate_blight(
        &self,
        player: PlayerId,
        count: u32,
        object: &rv1::CostObjectRef,
    ) -> Result<(), EngineError> {
        if count == 0
            || !self.can_blight_creature(player, object.object_id)
            || self
                .state
                .zone_change_generation
                .get(&object.object_id)
                .copied()
                .unwrap_or(0)
                != object.zone_change_generation
        {
            return Err(EngineError::Illegal("illegal or stale Blight creature"));
        }
        Ok(())
    }

    pub(super) fn complete_blight(
        &mut self,
        player: PlayerId,
        count: u32,
        creature: Option<ObjectId>,
    ) -> BlightReceipt {
        let creature = creature.map(|oid| TriggerObjectRef {
            object_id: oid,
            zone_change_generation: self
                .state
                .zone_change_generation
                .get(&oid)
                .copied()
                .unwrap_or(0),
            controller_at_event: player,
        });
        if let Some(object) = &creature {
            self.place_counters(object.object_id, CounterKind::MinusOneMinusOne, count);
        }
        BlightReceipt {
            player,
            count,
            creature,
        }
    }

    pub(super) fn blight_cost_choice(
        &self,
        player: PlayerId,
        cost_index: usize,
        count: u32,
    ) -> rv1::LegalCostChoice {
        let candidate_ids = self.blight_candidates(player);
        rv1::LegalCostChoice {
            cost_index: cost_index as u32,
            zone: rv1::CostChoiceZone::Battlefield as i32,
            min: 1,
            max: 1,
            kind: rv1::CostChoiceKind::Blight as i32,
            blight_count: count,
            counter_removal: None,
            candidate_objects: candidate_ids
                .iter()
                .map(|&oid| rv1::CostObjectRef {
                    object_id: oid,
                    zone_change_generation: self
                        .state
                        .zone_change_generation
                        .get(&oid)
                        .copied()
                        .unwrap_or(0),
                })
                .collect(),
            candidate_ids,
        }
    }
}
