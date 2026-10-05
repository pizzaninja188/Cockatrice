//! CR 404.3: arrange actual simultaneous graveyard arrivals before resuming resolution.
use super::*;

#[derive(serde::Serialize, Debug, Clone)]
pub(super) struct SacrificeDeparture {
    pub source: TriggerSourceSnapshot,
    pub was_creature: bool,
    pub died: bool,
}

/// The committed sacrifice event survives its owners' sequential ordering choices.
#[derive(serde::Serialize, Debug, Clone)]
pub struct PendingMassSacrifice {
    pub(super) zone: zone_events::ZoneEventBatch,
    pub(super) departures: Vec<SacrificeDeparture>,
    /// Validation is reduced by player departure; the committed rules event is immutable.
    pub(super) live_validation: Vec<zone_events::ZoneChangeReceipt>,
    pub(super) remaining: VecDeque<(PlayerId, Vec<ObjectId>)>,
}

impl GameEngine {
    pub(super) fn begin_mass_sacrifice_order(
        &mut self,
        item: StackItem,
        zone: zone_events::ZoneEventBatch,
        departures: Vec<SacrificeDeparture>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> bool {
        let mut groups = self
            .state
            .players
            .iter()
            .map(|player| {
                let arrivals = zone
                    .moves
                    .iter()
                    .filter(|receipt| {
                        receipt.destination == Zone::Graveyard
                            && receipt.before.owner == player.id
                            && self.state.is_card_object(receipt.before.object_id)
                    })
                    .map(|receipt| receipt.before.object_id)
                    .collect::<Vec<_>>();
                (player.id, arrivals)
            })
            .filter(|(_, arrivals)| arrivals.len() > 1)
            .collect::<Vec<_>>();
        groups.sort_by_key(|(owner, _)| self.state.apnap_rank(*owner));
        self.advance_mass_sacrifice_order(
            Some(ParkedStackResolution::new(item)),
            PendingMassSacrifice {
                live_validation: zone.moves.clone(),
                zone,
                departures,
                remaining: groups.into(),
            },
            events,
        )
    }

    fn advance_mass_sacrifice_order(
        &mut self,
        stack: Option<ParkedStackResolution>,
        mut sacrifice: PendingMassSacrifice,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> bool {
        self.reconcile_combat_characteristics(events);
        sacrifice
            .remaining
            .retain(|(_, candidates)| candidates.len() > 1);
        let Some((owner, candidates)) = sacrifice.remaining.pop_front() else {
            self.finish_mass_sacrifice_events(sacrifice, events);
            return false;
        };
        let count = candidates.len() as u32;
        let source_object_id = stack.as_ref().map_or(0, |stack| stack.item.id);
        let prompt =
            "Order the cards entering your graveyard, oldest first; the last card is on top."
                .to_string();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: owner,
                    source_object_id,
                    prompt_text: prompt.clone(),
                    choice_kind: custom::ChoiceKind::GraveyardCards as i32,
                    candidate_object_ids: candidates.clone(),
                    candidate_card_ids: candidates
                        .iter()
                        .map(|oid| self.state.objects[oid].card_id.clone())
                        .collect(),
                    candidate_names: candidates
                        .iter()
                        .map(|&oid| events::object_display_name(&self.state, self.registry, oid))
                        .collect(),
                    min: count,
                    max: count,
                    ordered: true,
                    candidate_source_zones: vec![
                        rv1::ChoiceCandidateSourceZone::Graveyard as i32;
                        candidates.len()
                    ],
                    ..Default::default()
                },
            )),
        });
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player: owner,
            presentation: PendingResolutionPresentation {
                source_object_id,
                candidates,
                min: count,
                max: count,
                ordered: true,
                prompt,
                choice_kind: custom::ChoiceKind::GraveyardCards,
                unique_names: false,
            },
            continuation: ResolutionContinuation::MassSacrificeGraveyardOrder {
                stack,
                sacrifice: Box::new(sacrifice),
            },
        });
        true
    }

    fn finish_mass_sacrifice_events(
        &mut self,
        sacrifice: PendingMassSacrifice,
        out: &mut Vec<rv1::RuledEvent>,
    ) {
        let mut triggers = sacrifice
            .departures
            .into_iter()
            .flat_map(|departure| {
                let player = departure.source.controller;
                sacrifice_events(
                    departure.source,
                    departure.was_creature,
                    player,
                    departure.died,
                )
            })
            .collect::<Vec<_>>();
        triggers.push(GameEvent::ZoneChanges(sacrifice.zone));
        self.fire_triggers(&triggers, out);
    }

    pub(super) fn finish_mass_sacrifice_graveyard_order(
        &mut self,
        pending: PendingResolution,
        chosen: &[ObjectId],
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::MassSacrificeGraveyardOrder { stack, sacrifice } =
            &pending.continuation
        else {
            return Err(EngineError::Illegal("mass sacrifice continuation missing"));
        };
        // Revalidate all surviving arrivals, including replacement exits, without erasing history.
        let current = sacrifice.live_validation.iter().all(|receipt| {
            self.state
                .objects
                .get(&receipt.before.object_id)
                .is_some_and(|object| {
                    object.zone == receipt.destination
                        && object.owner == receipt.before.owner
                        && self.state.zone_change_generation.get(&object.id).copied()
                            == Some(receipt.destination_generation)
                })
        });
        let Some(owner_index) = self.state.player_idx(pending.deciding_player) else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("graveyard owner missing"));
        };
        let positions = self.state.players[owner_index]
            .graveyard
            .iter()
            .enumerate()
            .filter_map(|(position, oid)| {
                pending
                    .presentation
                    .candidates
                    .contains(oid)
                    .then_some(position)
            })
            .collect::<Vec<_>>();
        if !current || positions.len() != chosen.len() {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "simultaneous graveyard arrivals became stale",
            ));
        }
        let stack = stack.clone();
        let sacrifice = *sacrifice.clone();
        for (position, &oid) in positions.into_iter().zip(chosen) {
            self.state.players[owner_index].graveyard[position] = oid;
        }
        let mut events = Vec::new();
        if self.advance_mass_sacrifice_order(stack.clone(), sacrifice, &mut events) {
            return Ok(events::finish_with_events(self, events));
        }
        self.complete_mass_sacrifice_order(stack, events)
    }

    fn complete_mass_sacrifice_order(
        &mut self,
        stack: Option<ParkedStackResolution>,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.drain_immediate_observer_actions(stack.clone(), &mut events)? {
            return Ok(events::finish_with_events(self, events));
        }
        if let Some(stack) = stack {
            self.complete_parked_resolution_with_previous(
                stack.item,
                stack.resume_effect_index,
                stack.previous_result,
                events,
            )
        } else {
            Ok(events::finish_with_events(self, events))
        }
    }

    pub(super) fn refresh_mass_sacrifice_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(mut pending) = self.state.pending_resolution.clone() else {
            return Ok(());
        };
        let ResolutionContinuation::MassSacrificeGraveyardOrder { stack, sacrifice } =
            &mut pending.continuation
        else {
            return Ok(());
        };
        let live_owners: HashSet<_> = self
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .map(|player| player.id)
            .collect();
        sacrifice
            .live_validation
            .retain(|receipt| live_owners.contains(&receipt.before.owner));
        let live_ids: HashSet<_> = sacrifice
            .live_validation
            .iter()
            .map(|receipt| receipt.before.object_id)
            .collect();
        sacrifice.remaining.retain_mut(|(owner, ids)| {
            ids.retain(|oid| live_ids.contains(oid));
            live_owners.contains(owner) && ids.len() > 1
        });
        let current = sacrifice.live_validation.iter().all(|receipt| {
            self.state
                .objects
                .get(&receipt.before.object_id)
                .is_some_and(|object| {
                    object.zone == receipt.destination
                        && object.owner == receipt.before.owner
                        && self.state.zone_change_generation.get(&object.id).copied()
                            == Some(receipt.destination_generation)
                })
        });
        if !current {
            let stack = stack.clone();
            let sacrifice = *sacrifice.clone();
            self.state.pending_resolution = None;
            self.finish_mass_sacrifice_events(sacrifice, events);
            return self.abandon_participating_resolution(stack, events);
        }
        if live_owners.contains(&pending.deciding_player) {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }
        let stack = stack.clone();
        let sacrifice = *sacrifice.clone();
        self.state.pending_resolution = None;
        let mut resumed = Vec::new();
        if !self.advance_mass_sacrifice_order(stack.clone(), sacrifice, &mut resumed) {
            resumed = self.complete_mass_sacrifice_order(stack, resumed)?.events;
        }
        events.extend(resumed);
        Ok(())
    }
}
