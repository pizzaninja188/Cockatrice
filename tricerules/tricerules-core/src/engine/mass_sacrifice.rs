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
            ParkedStackResolution::new(item),
            PendingMassSacrifice {
                zone,
                departures,
                remaining: groups.into(),
            },
            events,
        )
    }

    fn advance_mass_sacrifice_order(
        &mut self,
        stack: ParkedStackResolution,
        mut sacrifice: PendingMassSacrifice,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> bool {
        let Some((owner, candidates)) = sacrifice.remaining.pop_front() else {
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
            self.fire_triggers(&triggers);
            return false;
        };
        let count = candidates.len() as u32;
        let prompt =
            "Order the cards entering your graveyard, oldest first; the last card is on top."
                .to_string();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: owner,
                    source_object_id: stack.item.id,
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
                source_object_id: stack.item.id,
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
        // Revalidate the entire committed event, including other owners and replacement exits.
        let current = sacrifice.zone.moves.iter().all(|receipt| {
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
        if self.drain_immediate_observer_actions(Some(stack.clone()), &mut events)? {
            return Ok(events::finish_with_events(self, events));
        }
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            stack.previous_result,
            events,
        )
    }
}
