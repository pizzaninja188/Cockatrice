use super::*;

impl GameEngine {
    /// CR 701.18: the controller submitted their library search choice. Move the found card to
    /// the declared destination, optionally reveal it publicly, then optionally shuffle.
    pub(super) fn finish_library_search(
        &mut self,
        pending: PendingResolution,
        chosen: &[u32],
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, destination, shuffle, reveal) = match &pending.continuation {
            ResolutionContinuation::SearchLibrary {
                stack,
                destination,
                shuffle,
                reveal,
            } => (stack.clone(), *destination, *shuffle, *reveal),
            _ => return Err(EngineError::Illegal("library-search continuation missing")),
        };
        let controller = stack.item.controller;

        let mut ev = vec![];

        if chosen.is_empty() {
            ev.push(ev_log(format!("P{controller} finds no card.")));
            if shuffle {
                crate::engine::shuffle_player_library_for_current_command(
                    &mut self.state,
                    controller,
                );
                ev.push(ev_log(format!("P{controller} shuffles their library.")));
            }
        } else {
            let oid = chosen[0];
            let card_name = self
                .state
                .objects
                .get(&oid)
                .and_then(|o| self.registry.get(&o.card_id))
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "card".to_string());

            match destination {
                SearchDestination::Hand => {
                    // Move to hand first, then shuffle.
                    if let Some(idx) = self.state.player_idx(controller) {
                        self.state.players[idx].library.retain(|&x| x != oid);
                        self.state.players[idx].hand.push(oid);
                    }
                    if let Some(o) = self.state.objects.get_mut(&oid) {
                        o.zone = Zone::Hand;
                    }
                    if reveal {
                        ev.push(ev_log(format!("P{controller} reveals {card_name}.")));
                        ev.push(ev_log(format!(
                            "P{controller} puts {card_name} into their hand."
                        )));
                    } else {
                        ev.push(ev_log_private(
                            format!("P{controller} puts {card_name} into their hand."),
                            controller,
                        ));
                        ev.push(ev_log_hidden_from(
                            format!("P{controller} puts a card into their hand."),
                            controller,
                        ));
                    }
                    if shuffle {
                        crate::engine::shuffle_player_library_for_current_command(
                            &mut self.state,
                            controller,
                        );
                        ev.push(ev_log(format!("P{controller} shuffles their library.")));
                    }
                }
                SearchDestination::TopOfLibrary => {
                    // Oracle: "then shuffle and put that card on top" — shuffle first, then put on top.
                    if let Some(idx) = self.state.player_idx(controller) {
                        self.state.players[idx].library.retain(|&x| x != oid);
                    }
                    if shuffle {
                        crate::engine::shuffle_player_library_for_current_command(
                            &mut self.state,
                            controller,
                        );
                        ev.push(ev_log(format!("P{controller} shuffles their library.")));
                    }
                    if let Some(idx) = self.state.player_idx(controller) {
                        self.state.players[idx].library.push_front(oid);
                    }
                    if let Some(o) = self.state.objects.get_mut(&oid) {
                        o.zone = Zone::Library;
                    }
                    if reveal {
                        ev.push(ev_log(format!("P{controller} reveals {card_name}.")));
                        ev.push(ev_log(format!(
                            "P{controller} puts {card_name} on top of their library."
                        )));
                    } else {
                        ev.push(ev_log_private(
                            format!("P{controller} puts {card_name} on top of their library."),
                            controller,
                        ));
                        ev.push(ev_log_hidden_from(
                            format!("P{controller} puts a card on top of their library."),
                            controller,
                        ));
                    }
                }
                SearchDestination::Battlefield { tapped } => {
                    let owner = self
                        .state
                        .objects
                        .get(&oid)
                        .map(|object| object.owner)
                        .ok_or(EngineError::Illegal("searched card is stale"))?;
                    let completion = BattlefieldEntryCompletion::LibrarySearch {
                        owner,
                        card_label: card_name.clone(),
                        shuffle,
                        resume_effect_index: stack.resume_effect_index,
                    };
                    match self.begin_battlefield_entry(
                        stack.item.clone(),
                        BattlefieldEntryEvent {
                            object_id: oid,
                            deciding_player: controller,
                            destination_controller: controller,
                            face_index: 0,
                            unlock_room_door: None,
                            chosen_x: 0,
                            player_life_snapshot: self.player_life_snapshot(),
                            tapped,
                            entry_counters: BTreeMap::new(),
                            applied_effects: Vec::new(),
                        },
                        completion,
                        &mut ev,
                    ) {
                        super::replacement::BattlefieldEntryProgress::Parked => {
                            return Ok(finish_with_events(self, ev));
                        }
                        super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                            self.commit_battlefield_entry(entry, None)?;
                        }
                    }
                    ev.push(ev_log(format!(
                        "P{controller} puts {card_name} onto the battlefield."
                    )));
                    ev.push(permanent_moved_event(
                        &self.state,
                        oid,
                        owner,
                        rv1::permanent_moved::Destination::Battlefield,
                    ));
                    if shuffle {
                        crate::engine::shuffle_player_library_for_current_command(
                            &mut self.state,
                            controller,
                        );
                        ev.push(ev_log(format!("P{controller} shuffles their library.")));
                    }
                }
            }
        }

        self.complete_parked_resolution(stack.item, stack.resume_effect_index, ev)
    }
}
