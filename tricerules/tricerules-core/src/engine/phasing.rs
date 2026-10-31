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

        let phase_out_under = root_characteristics.controller;
        let direct_attachment_snapshot = self
            .is_phasing_attachment(object_id)
            .then(|| self.snapshot_attachment_recipient(object_id))
            .flatten();

        // Walk the active attachment graph to a fixed point. Each child keeps its immediate
        // parent's incarnation, so a later phase-in cannot use an ultimate-root fallback if an
        // intermediate attachment has left and returned as a new object generation.
        let mut members = vec![(object_id, None)];
        let mut visited = BTreeSet::new();
        visited.insert(object_id);
        let mut cursor = 0;
        while cursor < members.len() {
            let parent_id = members[cursor].0;
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
                        && candidate.attached_to == Some(AttachmentRecipient::Object(parent_id))
                        && self.is_phasing_attachment(candidate_id))
                    .then_some(candidate_id)
                })
                .collect::<Vec<_>>();
            children.sort_unstable();
            for child_id in children {
                if visited.insert(child_id) {
                    members.push((
                        child_id,
                        Some(PhasedOutParentRef {
                            object_id: parent_id,
                            zone_change_generation: parent_generation,
                        }),
                    ));
                }
            }
            cursor += 1;
        }

        // Freeze every member's characteristics before any status changes. Phasing preserves
        // the same battlefield generation, so these receipts provide the otherwise-missing LKI.
        let cohort = members
            .into_iter()
            .map(|(member_id, parent_ref)| {
                let generation = self
                    .state
                    .zone_change_generation
                    .get(&member_id)
                    .copied()
                    .unwrap_or(0);
                self.characteristics(member_id)
                    .map(|characteristics| (member_id, parent_ref, generation, characteristics))
            })
            .collect::<Option<Vec<_>>>();
        let Some(cohort) = cohort else {
            return Ok(false);
        };
        if cohort
            .first()
            .is_none_or(|(member_id, _, _, _)| *member_id != object_id)
        {
            return Ok(false);
        }

        for (member_id, _, generation, characteristics) in &cohort {
            resolution::record_last_known_characteristics(
                &mut self.state,
                *member_id,
                *generation,
                characteristics.clone(),
            );
        }

        // CR 506.4 removes every member from the current combat before its active characteristics
        // disappear. The event is emitted once for the exact object IDs that were participants.
        let cohort_ids = cohort
            .iter()
            .map(|(member_id, _, _, _)| *member_id)
            .collect::<Vec<_>>();
        self.remove_combat_participants(&cohort_ids, events);

        // All members are marked before the index is rebuilt or any derived state is observed.
        let prepared_members = cohort
            .iter()
            .map(|(member_id, _, generation, _)| (*member_id, *generation))
            .collect::<Vec<_>>();
        for (member_id, parent_ref, _, _) in cohort {
            if let Some(member) = self.state.objects.get_mut(&member_id) {
                member.phase_status = Some(PhasedOutStatus {
                    phase_out_under,
                    phase_in_due_after_departed_turn: false,
                    phase_in_with: parent_ref,
                    direct_attachment_snapshot: if member_id == object_id {
                        direct_attachment_snapshot.clone()
                    } else {
                        None
                    },
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
