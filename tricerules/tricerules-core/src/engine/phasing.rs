//! Shared backend transition helpers for CR 702.26 phasing.

use super::*;
use crate::state::{PhasedOutAttachmentSnapshot, PhasedOutParentRef, PhasedOutStatus};

impl GameEngine {
    /// Phase out one legal chosen creature and its direct Aura/Equipment/Fortification cohort.
    /// This mutates status on the existing object incarnations; no zone transition is performed.
    pub(super) fn phase_out_chosen_creature(
        &mut self,
        object_id: ObjectId,
        effect_controller: PlayerId,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(object) = self.state.objects.get(&object_id) else {
            return Ok(false);
        };
        if object.zone != Zone::Battlefield || object.phase_status.is_some() {
            return Ok(false);
        }
        let Some(root_characteristics) = self.characteristics(object_id) else {
            return Ok(false);
        };
        if !root_characteristics.is_creature()
            || root_characteristics.controller == effect_controller
        {
            return Ok(false);
        }
        let generation = self
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0);
        self.phase_out_selected_permanents(
            &[crate::state::TriggerObjectRef {
                object_id,
                zone_change_generation: generation,
                controller_at_event: root_characteristics.controller,
            }],
            events,
        )
    }

    /// Phase out the current, exact-generation Ripples selection as one simultaneous batch.
    /// Every selected permanent is a root only when no selected ancestor also phases; the full
    /// active attachment closure is captured before any member changes status.
    pub(super) fn phase_out_proliferated_permanents(
        &mut self,
        selected: &[crate::state::TriggerObjectRef],
        effect_controller: PlayerId,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let controller_live = self
            .state
            .player_idx(effect_controller)
            .is_some_and(|index| !self.state.players[index].has_lost);
        if !controller_live {
            return Ok(());
        }
        let current = selected
            .iter()
            .copied()
            .filter(|reference| {
                self.state
                    .zone_change_generation
                    .get(&reference.object_id)
                    .copied()
                    .unwrap_or(0)
                    == reference.zone_change_generation
                    && self
                        .state
                        .objects
                        .get(&reference.object_id)
                        .is_some_and(|object| {
                            object.zone == Zone::Battlefield && object.phase_status.is_none()
                        })
                    && self.controller_of(reference.object_id) == Some(effect_controller)
            })
            .collect::<Vec<_>>();
        let _ = self.phase_out_selected_permanents(&current, events)?;
        Ok(())
    }

    fn phase_out_selected_permanents(
        &mut self,
        selected: &[crate::state::TriggerObjectRef],
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let mut selected_generations = std::collections::BTreeMap::new();
        for reference in selected {
            let generation = self
                .state
                .zone_change_generation
                .get(&reference.object_id)
                .copied()
                .unwrap_or(0);
            if generation != reference.zone_change_generation
                || !self
                    .state
                    .objects
                    .get(&reference.object_id)
                    .is_some_and(|object| {
                        object.zone == Zone::Battlefield && object.phase_status.is_none()
                    })
                || self.characteristics(reference.object_id).is_none()
            {
                continue;
            }
            selected_generations
                .entry(reference.object_id)
                .or_insert(reference.zone_change_generation);
        }
        if selected_generations.is_empty() {
            return Ok(false);
        }

        let roots = selected_generations
            .keys()
            .copied()
            .filter(|object_id| {
                !self.has_selected_phasing_ancestor(*object_id, &selected_generations)
            })
            .collect::<Vec<_>>();
        let mut members = std::collections::BTreeMap::new();
        for root_id in roots {
            let root_generation = selected_generations[&root_id];
            let Some(root_characteristics) = self.characteristics(root_id) else {
                continue;
            };
            let phase_out_under = root_characteristics.controller;
            let direct_attachment_snapshot = self
                .is_phasing_attachment(root_id)
                .then(|| self.snapshot_attachment_recipient(root_id))
                .flatten();
            members.insert(
                root_id,
                (
                    None,
                    root_generation,
                    root_characteristics,
                    phase_out_under,
                    direct_attachment_snapshot,
                ),
            );

            // Traverse from every root, preserving the immediate parent's exact incarnation for
            // each indirect member. Child ordering is stable, and no status changes occur here.
            let mut queue = vec![root_id];
            let mut cursor = 0;
            while cursor < queue.len() {
                let parent_id = queue[cursor];
                let parent_generation = self
                    .state
                    .zone_change_generation
                    .get(&parent_id)
                    .copied()
                    .unwrap_or(0);
                let mut children = self
                    .state
                    .objects
                    .iter()
                    .filter_map(|(&candidate_id, candidate)| {
                        (candidate.zone == Zone::Battlefield
                            && candidate.phase_status.is_none()
                            && candidate.attached_to
                                == Some(AttachmentRecipient::Object(parent_id))
                            && self.is_phasing_attachment(candidate_id))
                        .then_some(candidate_id)
                    })
                    .collect::<Vec<_>>();
                children.sort_unstable();
                for child_id in children {
                    if members.contains_key(&child_id) {
                        continue;
                    }
                    let Some(characteristics) = self.characteristics(child_id) else {
                        continue;
                    };
                    let generation = self
                        .state
                        .zone_change_generation
                        .get(&child_id)
                        .copied()
                        .unwrap_or(0);
                    members.insert(
                        child_id,
                        (
                            Some(PhasedOutParentRef {
                                object_id: parent_id,
                                zone_change_generation: parent_generation,
                            }),
                            generation,
                            characteristics,
                            phase_out_under,
                            None,
                        ),
                    );
                    queue.push(child_id);
                }
                cursor += 1;
            }
        }
        if members.is_empty() {
            return Ok(false);
        }

        // Freeze every member's characteristics before any status changes. Phasing preserves
        // each battlefield generation, so these receipts provide the otherwise-missing LKI.
        for (&member_id, (_, generation, characteristics, _, _)) in &members {
            resolution::record_last_known_characteristics(
                &mut self.state,
                member_id,
                *generation,
                characteristics.clone(),
            );
        }

        // CR 506.4 removes the complete selected/indirect cohort from combat in one operation.
        let member_ids = members.keys().copied().collect::<Vec<_>>();
        self.remove_combat_participants(&member_ids, events);

        // All members are marked before the index is rebuilt or any derived state is observed.
        let prepared_members = members
            .iter()
            .map(|(&member_id, (_, generation, _, _, _))| (member_id, *generation))
            .collect::<Vec<_>>();
        for (member_id, (parent_ref, _, _, phase_out_under, direct_attachment_snapshot)) in members
        {
            if let Some(member) = self.state.objects.get_mut(&member_id) {
                member.phase_status = Some(PhasedOutStatus {
                    phase_out_under,
                    phase_in_due_after_departed_turn: false,
                    phase_in_with: parent_ref,
                    direct_attachment_snapshot,
                });
            }
        }
        for (member_id, generation) in prepared_members {
            super::preparation::retire_active_prepared_exile_copy(
                &mut self.state,
                member_id,
                generation,
            );
        }
        self.reindex_battlefield_control(events)?;
        Ok(true)
    }

    fn has_selected_phasing_ancestor(
        &self,
        object_id: ObjectId,
        selected_generations: &std::collections::BTreeMap<ObjectId, u64>,
    ) -> bool {
        let mut cursor = object_id;
        let mut visited = std::collections::BTreeSet::new();
        loop {
            let Some(object) = self.state.objects.get(&cursor) else {
                return false;
            };
            if object.zone != Zone::Battlefield
                || object.phase_status.is_some()
                || !self.is_phasing_attachment(cursor)
            {
                return false;
            }
            let Some(AttachmentRecipient::Object(parent_id)) = object.attached_to else {
                return false;
            };
            if !visited.insert(parent_id) {
                return false;
            }
            let Some(parent) = self.state.objects.get(&parent_id) else {
                return false;
            };
            if parent.zone != Zone::Battlefield || parent.phase_status.is_some() {
                return false;
            }
            let parent_generation = self
                .state
                .zone_change_generation
                .get(&parent_id)
                .copied()
                .unwrap_or(0);
            if selected_generations.get(&parent_id) == Some(&parent_generation) {
                return true;
            }
            cursor = parent_id;
        }
    }

    /// Phase in due roots and the indirect attachments whose exact immediate parent also
    /// returns. Status is cleared for the complete return set before control is reindexed.
    pub(super) fn phase_in_due_permanents(
        &mut self,
        active_player: PlayerId,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let mut return_set = self
            .state
            .objects
            .values()
            .filter_map(|object| {
                let status = object.phase_status.as_ref()?;
                let departed_controller_turn_passed = self
                    .state
                    .player_idx(status.phase_out_under)
                    .is_some_and(|index| self.state.players[index].has_lost)
                    && status.phase_in_due_after_departed_turn;
                let active_controller_due = status.phase_out_under == active_player
                    && self
                        .state
                        .player_idx(status.phase_out_under)
                        .is_some_and(|index| !self.state.players[index].has_lost);
                (object.zone == Zone::Battlefield
                    && (active_controller_due || departed_controller_turn_passed)
                    && status.phase_in_with.is_none())
                .then_some(object.id)
            })
            .collect::<BTreeSet<_>>();

        if return_set.is_empty() {
            return Ok(());
        }

        // Only walk from due direct roots through exact parent incarnations. A descendant whose
        // parent left, or returned with a new generation, is intentionally left phased out.
        let mut return_order = return_set.iter().copied().collect::<Vec<_>>();
        let mut cursor = 0;
        while cursor < return_order.len() {
            let parent_id = return_order[cursor];
            let parent_is_currently_phased =
                self.state.objects.get(&parent_id).is_some_and(|parent| {
                    parent.zone == Zone::Battlefield && parent.phase_status.is_some()
                });
            if !parent_is_currently_phased {
                cursor += 1;
                continue;
            }
            let parent_generation = self
                .state
                .zone_change_generation
                .get(&parent_id)
                .copied()
                .unwrap_or(0);
            let mut children = self
                .state
                .objects
                .iter()
                .filter_map(|(&child_id, child)| {
                    let parent_ref = child.phase_status.as_ref()?.phase_in_with?;
                    (child.zone == Zone::Battlefield
                        && parent_ref.object_id == parent_id
                        && parent_ref.zone_change_generation == parent_generation)
                        .then_some(child_id)
                })
                .collect::<Vec<_>>();
            children.sort_unstable();
            for child_id in children {
                if return_set.insert(child_id) {
                    return_order.push(child_id);
                }
            }
            cursor += 1;
        }

        // A directly phased attachment that is itself an attachment can return only with the
        // exact recipient incarnation it had before phasing. Plan these link repairs before any
        // status is cleared, then apply them as part of the same simultaneous transition.
        let direct_attachment_updates = return_order
            .iter()
            .filter_map(|&object_id| {
                let status = self.state.objects.get(&object_id)?.phase_status.as_ref()?;
                if status.phase_in_with.is_some() {
                    return None;
                }
                let snapshot = status.direct_attachment_snapshot.as_ref()?;
                let recipient = self
                    .attachment_snapshot_recipient_is_current(snapshot)
                    .then_some(snapshot.recipient);
                Some((object_id, recipient))
            })
            .collect::<Vec<_>>();

        for object_id in &return_order {
            if let Some(object) = self.state.objects.get_mut(object_id) {
                object.phase_status = None;
            }
        }
        for (object_id, recipient) in direct_attachment_updates {
            if let Some(object) = self.state.objects.get_mut(&object_id) {
                object.attached_to = recipient;
            }
        }

        self.reindex_battlefield_control(events)?;
        for object_id in return_order {
            if self
                .state
                .prepared_permanents
                .get(&object_id)
                .is_some_and(|designation| {
                    designation.source_zone_change_generation
                        == self
                            .state
                            .zone_change_generation
                            .get(&object_id)
                            .copied()
                            .unwrap_or(0)
                        && designation.active_exile_copy.is_none()
                })
            {
                self.recreate_prepared_exile_copy_on_phase_in(object_id);
            }
        }
        Ok(())
    }

    fn attachment_snapshot_recipient_is_current(
        &self,
        snapshot: &PhasedOutAttachmentSnapshot,
    ) -> bool {
        match snapshot.recipient {
            AttachmentRecipient::Object(recipient_id) => {
                let Some(expected_zone) = snapshot.recipient_zone else {
                    return false;
                };
                let Some(expected_generation) = snapshot.recipient_zone_change_generation else {
                    return false;
                };
                self.state
                    .objects
                    .get(&recipient_id)
                    .is_some_and(|recipient| {
                        recipient.zone == expected_zone
                            && self
                                .state
                                .zone_change_generation
                                .get(&recipient_id)
                                .copied()
                                .unwrap_or(0)
                                == expected_generation
                    })
            }
            AttachmentRecipient::Player(player_id) => self
                .state
                .players
                .iter()
                .any(|player| player.id == player_id && !player.has_lost),
        }
    }

    fn is_phasing_attachment(&self, object_id: ObjectId) -> bool {
        self.characteristics(object_id)
            .is_some_and(|characteristics| {
                characteristics.has_type("Aura")
                    || characteristics.has_type("Equipment")
                    || characteristics.has_type("Fortification")
            })
    }

    fn snapshot_attachment_recipient(
        &self,
        object_id: ObjectId,
    ) -> Option<PhasedOutAttachmentSnapshot> {
        let recipient = self.state.objects.get(&object_id)?.attached_to?;
        let (recipient_zone, recipient_zone_change_generation) = match recipient {
            AttachmentRecipient::Object(recipient_id) => {
                let object = self.state.objects.get(&recipient_id)?;
                (
                    Some(object.zone),
                    Some(
                        self.state
                            .zone_change_generation
                            .get(&recipient_id)
                            .copied()
                            .unwrap_or(0),
                    ),
                )
            }
            AttachmentRecipient::Player(_) => (None, None),
        };
        Some(PhasedOutAttachmentSnapshot {
            recipient,
            recipient_zone,
            recipient_zone_change_generation,
        })
    }
}
