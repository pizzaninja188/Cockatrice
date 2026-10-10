use super::events::ev_log;
use super::resolution::{
    commit_zone_move, consume_regen_shield, destroy_permanent, permanent_moved_event,
    prepare_zone_move, PreparedZoneMove,
};
use super::*;
use crate::state::{
    PendingStateBasedActionAnswer, PendingStateBasedActionChoice, PendingStateBasedActionChoices,
};

fn prepare_state_based_zone_move(
    prepared: &mut HashMap<ObjectId, PreparedZoneMove>,
    state: &GameState,
    registry: &'static CardRegistry,
    object_id: ObjectId,
    destination: Zone,
) -> Result<(), EngineError> {
    let movement = prepare_zone_move(state, registry, object_id, destination, None)?.ok_or(
        EngineError::Illegal("state-based action zone move could not be prepared"),
    )?;
    if prepared.insert(object_id, movement).is_some() {
        return Err(EngineError::Illegal(
            "duplicate state-based action zone move",
        ));
    }
    Ok(())
}

fn commit_prepared_state_based_zone_move(
    state: &mut GameState,
    registry: &'static CardRegistry,
    movement: PreparedZoneMove,
) -> Result<bool, EngineError> {
    let object_id = movement.object_id();
    if let Some(fact) = commit_zone_move(state, registry, movement)? {
        state
            .turn_history
            .current
            .permanent_cards_entered_graveyard
            .push(fact);
    }
    Ok(state
        .objects
        .get(&object_id)
        .is_some_and(|object| object.zone == Zone::Graveyard))
}

impl GameEngine {
    /// CR 704.4: state-based actions are checked and performed repeatedly until a check finds
    /// nothing left to do. Stops while the current CR 704.3 choice set is being collected.
    pub(super) fn apply_sbas(&mut self, out: &mut Vec<rv1::RuledEvent>) -> Result<(), EngineError> {
        self.apply_sbas_and_report(out).map(|_| ())
    }

    /// Run the full state-based-action loop and report whether a CR 704 action was performed.
    /// Bookkeeping that settles derived indexes is deliberately excluded from the result.
    pub(super) fn apply_sbas_and_report(
        &mut self,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        if self.state.is_terminal() {
            return Ok(false);
        }
        let mut performed = false;
        while !self.state.is_terminal() && self.state.pending_resolution.is_none() {
            let (changed, performed_this_pass) = self.apply_sbas_once(out)?;
            performed |= performed_this_pass;
            if !changed {
                break;
            }
        }
        self.check_millennium_calendar_state_triggers();
        self.debug_assert_battlefield_control_index();
        Ok(performed)
    }

    /// The battlefield lists are the *control* index (see [`GameObject::controller`]). Every zone
    /// mutation has to keep them in sync, and getting it wrong produces a ghost permanent that
    /// still blocks and still gets SBA-checked — silent, and far from its cause. Assert the
    /// invariant in both directions once the board has settled.
    ///
    /// Debug-only: this is O(board) per settled SBA loop, and the release build must not pay for
    /// it (see the `performance.rs` wall-time bound).
    fn debug_assert_battlefield_control_index(&self) {
        #[cfg(debug_assertions)]
        {
            for player in &self.state.players {
                for oid in &player.battlefield {
                    let Some(object) = self.state.objects.get(oid) else {
                        panic!("oid {oid} in P{}'s battlefield has no object", player.id);
                    };
                    assert_eq!(
                        object.zone,
                        Zone::Battlefield,
                        "oid {oid} is in P{}'s battlefield list but its zone is {:?}",
                        player.id,
                        object.zone
                    );
                    assert_eq!(
                        object.controller, player.id,
                        "oid {oid} is in P{}'s battlefield list but is controlled by P{}",
                        player.id, object.controller
                    );
                }
            }
            for (oid, object) in &self.state.objects {
                if object.zone != Zone::Battlefield {
                    continue;
                }
                let listed = self
                    .state
                    .players
                    .iter()
                    .any(|p| p.id == object.controller && p.battlefield.contains(oid));
                assert!(
                    listed,
                    "oid {oid} is on the battlefield controlled by P{} but is not in that \
                     player's battlefield list",
                    object.controller
                );
            }
        }
    }

    /// One state-based-action pass (CR 704.5). Returns `true` if it changed game state.
    pub(super) fn apply_sbas_once(
        &mut self,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(bool, bool), EngineError> {
        self.apply_sbas_once_with_choices(out, None)
    }

    pub(in crate::engine) fn apply_sbas_once_with_choices(
        &mut self,
        out: &mut Vec<rv1::RuledEvent>,
        choice_set: Option<PendingStateBasedActionChoices>,
    ) -> Result<(bool, bool), EngineError> {
        if self.state.is_terminal() {
            return Ok((false, false));
        }
        let lost_players = self
            .state
            .players
            .iter()
            .filter(|player| player.has_lost)
            .map(|player| player.id)
            .collect::<HashSet<_>>();
        let effect_count = self.state.continuous_effects.len();
        self.state.continuous_effects.retain(|effect| {
            !matches!(
                effect.duration,
                EffectDuration::UntilEndOfNextTurn { player, .. }
                    if lost_players.contains(&player)
            )
        });
        let mut changed = self.state.continuous_effects.len() != effect_count;
        changed |= self.reindex_battlefield_control(out);
        changed |= self.refresh_enduring_story_designations();
        let removed_emblems = self
            .state
            .static_emblems
            .iter()
            .filter(|emblem| lost_players.contains(&emblem.controller))
            .map(|emblem| emblem.object_id)
            .collect::<HashSet<_>>();
        if !removed_emblems.is_empty() {
            self.state
                .static_emblems
                .retain(|emblem| !removed_emblems.contains(&emblem.object_id));
            self.state.continuous_effects.retain(|effect| {
                effect
                    .source_id
                    .is_none_or(|source| !removed_emblems.contains(&source))
            });
            changed = true;
        }
        if self.state.pending_resolution.is_some() {
            return Ok((changed, false));
        }
        if let Some(choices) = choice_set.as_ref() {
            if !self.state_based_action_choices_are_current(choices) {
                return Err(EngineError::Illegal("stale state-based action choice set"));
            }
        } else {
            let choices = self.collect_state_based_action_choices();
            if !choices.is_empty() {
                let choice_set = PendingStateBasedActionChoices {
                    choices,
                    next_choice_index: 0,
                    answers: Vec::new(),
                };
                self.publish_state_based_action_choice(choice_set, out)?;
                return Ok((changed, false));
            }
        }
        let mut performed = false;
        // CR 704.5c: a player with ten or more poison counters loses. All players at the
        // threshold lose in the same SBA pass before their departures are reconciled.
        for player in &mut self.state.players {
            if !player.has_lost
                && player
                    .counters
                    .get(&CounterKind::Poison)
                    .copied()
                    .unwrap_or(0)
                    >= 10
            {
                player.has_lost = true;
                changed = true;
                performed = true;
            }
        }
        let zone_snapshot = self.snapshot_zone_event();
        let mut leaves: Vec<(TriggerSourceSnapshot, bool, bool)> = Vec::new();
        let mut tap_events = Vec::new();
        let mut commander_zone_moved = false;
        let mut candidate_ids: Vec<ObjectId> = self
            .state
            .objects
            .iter()
            .filter(|(_, object)| object.zone == Zone::Battlefield)
            .map(|(id, _)| *id)
            .collect();
        candidate_ids.sort_unstable();

        // CR 704.5f/704.5i: toughness-0 creatures and zero-loyalty planeswalkers are put into
        // their owners' graveyards. Neither action is destruction, so regeneration and
        // indestructible do not apply.
        let mut to_destroy_t0 = Vec::new();
        // CR 704.5g/704.5h: lethal-damage deaths — regeneration shields apply here.
        let mut to_destroy_lethal = Vec::new();
        // CR 704.5s: a Saga is sacrificed only after all chapter abilities from its current
        // generation have left every staging, choice, and stack container.
        let mut sagas_to_sacrifice = Vec::new();
        for id in candidate_ids {
            let Some(characteristics) = self.characteristics(id) else {
                continue;
            };
            let Some(o) = self.state.objects.get(&id) else {
                continue;
            };
            if characteristics.has_type("Planeswalker")
                && o.counter_count(CounterKind::Loyalty) == 0
            {
                to_destroy_t0.push(id);
                continue;
            }
            if characteristics.has_type("Battle") && o.counter_count(CounterKind::Defense) == 0 {
                let generation = self
                    .state
                    .zone_change_generation
                    .get(&id)
                    .copied()
                    .unwrap_or(0);
                if !self.siege_defeat_trigger_active(id, generation) {
                    to_destroy_t0.push(id);
                }
                continue;
            }
            if self
                .saga_final_chapter(id)
                .is_some_and(|final_chapter| o.counter_count(CounterKind::Lore) >= final_chapter)
            {
                let generation = self
                    .state
                    .zone_change_generation
                    .get(&id)
                    .copied()
                    .unwrap_or(0);
                if !self.saga_chapter_trigger_active(id, generation) {
                    sagas_to_sacrifice.push(id);
                }
            }
            let Some(eff_t) = characteristics.toughness else {
                continue;
            };
            let indestructible = characteristics.has_keyword(Keyword::Indestructible);
            // CR 704.5f: toughness 0 — still dies even with indestructible.
            if eff_t == 0 {
                to_destroy_t0.push(id);
            } else if !indestructible && (o.damage >= eff_t || o.deathtouch_damage) {
                to_destroy_lethal.push(id);
            }
        }
        // CR 603.6/603.10: snapshot every object that will die in this simultaneous SBA set
        // before moving any of them. This preserves granted abilities even if their granting Aura
        // is another member of the same destruction set.
        let death_snapshots: HashMap<ObjectId, (TriggerSourceSnapshot, bool)> = to_destroy_t0
            .iter()
            .chain(&to_destroy_lethal)
            .filter_map(|&id| {
                let was_creature = self
                    .characteristics(id)
                    .is_some_and(|value| value.is_creature());
                self.trigger_source_snapshot(id)
                    .map(|snapshot| (id, (snapshot, was_creature)))
            })
            .collect();
        // Freeze every zone move in this CR 704.3 set before removing counters or committing the
        // first departure. A source's last-known information must see the same battlefield even
        // when an earlier object (such as an anthem) leaves first.
        let mut prepared_zone_moves = HashMap::new();
        let mut planned_zone_move_ids = HashSet::new();
        for &id in to_destroy_t0.iter().chain(&to_destroy_lethal) {
            if to_destroy_lethal.contains(&id)
                && self
                    .state
                    .objects
                    .get(&id)
                    .is_some_and(|object| object.regeneration_shields > 0)
            {
                continue;
            }
            if planned_zone_move_ids.insert(id) {
                prepare_state_based_zone_move(
                    &mut prepared_zone_moves,
                    &self.state,
                    self.registry,
                    id,
                    Zone::Graveyard,
                )?;
            }
        }
        for &id in &sagas_to_sacrifice {
            if planned_zone_move_ids.insert(id) {
                prepare_state_based_zone_move(
                    &mut prepared_zone_moves,
                    &self.state,
                    self.registry,
                    id,
                    Zone::Graveyard,
                )?;
            }
        }
        if let Some(choices) = &choice_set {
            for (choice, answer) in choices.choices.iter().zip(&choices.answers) {
                match (choice, answer) {
                    (
                        PendingStateBasedActionChoice::CommanderZone { object_id, .. },
                        PendingStateBasedActionAnswer::CommanderZone {
                            move_to_command_zone: true,
                        },
                    ) if planned_zone_move_ids.insert(*object_id) => {
                        prepare_state_based_zone_move(
                            &mut prepared_zone_moves,
                            &self.state,
                            self.registry,
                            *object_id,
                            Zone::Command,
                        )?;
                    }
                    (
                        PendingStateBasedActionChoice::LegendKeep { candidates, .. },
                        PendingStateBasedActionAnswer::LegendKeep { object_id: keep_id },
                    ) => {
                        for &(object_id, _) in candidates {
                            if object_id != *keep_id
                                && self
                                    .state
                                    .objects
                                    .get(&object_id)
                                    .is_some_and(|object| object.zone == Zone::Battlefield)
                                && planned_zone_move_ids.insert(object_id)
                            {
                                prepare_state_based_zone_move(
                                    &mut prepared_zone_moves,
                                    &self.state,
                                    self.registry,
                                    object_id,
                                    Zone::Graveyard,
                                )?;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        // Counter cancellation and deaths are one simultaneous SBA set. Capture all death decisions
        // and last-known characteristics before removing either kind of counter.
        for o in self.state.objects.values_mut() {
            if o.zone != Zone::Battlefield {
                continue;
            }
            let plus = o.counter_count(CounterKind::PlusOnePlusOne);
            let minus = o.counter_count(CounterKind::MinusOneMinusOne);
            let pairs = plus.min(minus);
            if pairs > 0 {
                o.set_counter(CounterKind::PlusOnePlusOne, plus - pairs);
                o.set_counter(CounterKind::MinusOneMinusOne, minus - pairs);
                changed = true;
                performed = true;
            }
        }

        // Toughness-0: bypass regeneration (CR 704.5f — not a "destroy" trigger).
        // CR 702.2b / 704.5h look only for deathtouch damage dealt since the previous SBA
        // check. Preserve the decisions collected above, then expire the history bit on every
        // battlefield object before this pass performs its actions. In particular, an
        // indestructible creature must not die during a later check merely because it lost
        // indestructible after surviving old deathtouch damage.
        for object in self.state.objects.values_mut() {
            if object.zone == Zone::Battlefield {
                object.deathtouch_damage = false;
            }
        }
        for id in to_destroy_t0 {
            let owner = self.state.objects.get(&id).map(|o| o.owner);
            let movement = prepared_zone_moves
                .remove(&id)
                .expect("toughness-zero SBA move was prepared from the frozen state");
            let died =
                commit_prepared_state_based_zone_move(&mut self.state, self.registry, movement)?;
            changed = true;
            performed = true;
            if let Some(owner_id) = owner {
                out.push(permanent_moved_event(
                    &self.state,
                    id,
                    owner_id,
                    rv1::permanent_moved::Destination::Graveyard,
                ));
            }
            if let Some((source, was_creature)) = death_snapshots.get(&id).cloned() {
                leaves.push((source, was_creature, died));
            }
        }
        // Lethal-damage destroy: CR 614.8 / 701.19 regeneration shields apply before destruction.
        for id in to_destroy_lethal {
            let owner = self.state.objects.get(&id).map(|o| o.owner);
            let snapshot = death_snapshots.get(&id).cloned();
            let (regenerated, tap_event) = consume_regen_shield(self, id, out);
            if regenerated {
                changed = true;
                performed = true;
                tap_events.extend(tap_event);
                let name = snapshot
                    .as_ref()
                    .map(|(source, _)| source.card_id.as_str())
                    .unwrap_or("creature");
                out.push(super::events::ev_log(format!("{name} regenerates.")));
            } else {
                let movement = prepared_zone_moves
                    .remove(&id)
                    .expect("lethal-damage SBA move was prepared from the frozen state");
                let died = commit_prepared_state_based_zone_move(
                    &mut self.state,
                    self.registry,
                    movement,
                )?;
                changed = true;
                performed = true;
                if let Some(owner_id) = owner {
                    out.push(permanent_moved_event(
                        &self.state,
                        id,
                        owner_id,
                        rv1::permanent_moved::Destination::Graveyard,
                    ));
                }
                if let Some((source, was_creature)) = snapshot {
                    leaves.push((source, was_creature, died));
                }
            }
        }

        let mut saga_leaves = Vec::new();
        for id in sagas_to_sacrifice {
            let Some(object) = self.state.objects.get(&id) else {
                continue;
            };
            if object.zone != Zone::Battlefield {
                continue;
            }
            let owner = object.owner;
            let controller = object.controller;
            let was_creature = self
                .characteristics(id)
                .is_some_and(|value| value.is_creature());
            let Some(source) = self.trigger_source_snapshot(id) else {
                continue;
            };
            let movement = prepared_zone_moves
                .remove(&id)
                .expect("Saga sacrifice was prepared from the frozen state");
            let died =
                commit_prepared_state_based_zone_move(&mut self.state, self.registry, movement)?;
            changed = true;
            performed = true;
            out.push(permanent_moved_event(
                &self.state,
                id,
                owner,
                rv1::permanent_moved::Destination::Graveyard,
            ));
            saga_leaves.push((source, was_creature, controller, died));
        }

        // All choices were collected from the same pre-action state. Commit accepted Commander
        // moves and legend-rule outcomes before dispatching any departures from this SBA set.
        if let Some(choice_set) = &choice_set {
            for (choice, answer) in choice_set.choices.iter().zip(&choice_set.answers) {
                match (choice, answer) {
                    (
                        PendingStateBasedActionChoice::CommanderZone {
                            object_id,
                            owner,
                            zone_change_generation,
                            ..
                        },
                        PendingStateBasedActionAnswer::CommanderZone {
                            move_to_command_zone,
                        },
                    ) => {
                        let prior = self
                            .state
                            .commander_sba_checked_generations
                            .insert(*object_id, *zone_change_generation);
                        changed |= prior != Some(*zone_change_generation);
                        if *move_to_command_zone {
                            let movement = prepared_zone_moves
                                .remove(object_id)
                                .expect("accepted Commander SBA move was prepared");
                            commit_prepared_state_based_zone_move(
                                &mut self.state,
                                self.registry,
                                movement,
                            )?;
                            commander_zone_moved = true;
                            out.push(permanent_moved_event(
                                &self.state,
                                *object_id,
                                *owner,
                                rv1::permanent_moved::Destination::Command,
                            ));
                            changed = true;
                            performed = true;
                        }
                    }
                    (
                        PendingStateBasedActionChoice::LegendKeep { candidates, .. },
                        PendingStateBasedActionAnswer::LegendKeep { object_id: keep_id },
                    ) => {
                        for &(object_id, _) in candidates {
                            if object_id == *keep_id
                                || !self
                                    .state
                                    .objects
                                    .get(&object_id)
                                    .is_some_and(|object| object.zone == Zone::Battlefield)
                            {
                                continue;
                            }
                            let owner = self
                                .state
                                .objects
                                .get(&object_id)
                                .map(|object| object.owner);
                            let source = zone_snapshot.source(object_id);
                            let was_creature = source.as_ref().is_some_and(|source| {
                                source.types.iter().any(|type_name| type_name == "Creature")
                            });
                            let Some(movement) = prepared_zone_moves.remove(&object_id) else {
                                continue;
                            };
                            let died = commit_prepared_state_based_zone_move(
                                &mut self.state,
                                self.registry,
                                movement,
                            )?;
                            if let Some(owner_id) = owner {
                                out.push(permanent_moved_event(
                                    &self.state,
                                    object_id,
                                    owner_id,
                                    rv1::permanent_moved::Destination::Graveyard,
                                ));
                            }
                            if let Some(source) = source {
                                leaves.push((source, was_creature, died));
                            }
                            changed = true;
                            performed = true;
                        }
                    }
                    _ => return Err(EngineError::Illegal("mismatched state-based action answer")),
                }
            }
        }

        if !leaves.is_empty()
            || !saga_leaves.is_empty()
            || !tap_events.is_empty()
            || commander_zone_moved
        {
            let mut trigger_events = tap_events;
            trigger_events.extend(leaves.into_iter().flat_map(|(source, was_creature, died)| {
                leaves_and_dies_events(source, was_creature, died)
            }));
            trigger_events.extend(saga_leaves.into_iter().flat_map(
                |(source, was_creature, controller, died)| {
                    sacrifice_events(source, was_creature, controller, died)
                },
            ));
            self.fire_zone_triggers(zone_snapshot, trigger_events, out);
        }

        // CR 704.5n/p: illegal Equipment and permanents that are no longer an Aura, Equipment,
        // or Fortification become unattached while remaining on the battlefield.
        let attachments_to_unattach: Vec<ObjectId> = self
            .state
            .objects
            .iter()
            .filter(|(_, eq)| {
                eq.zone == Zone::Battlefield
                    && eq.attached_to.is_some_and(|recipient| {
                        self.characteristics(eq.id).is_some_and(|value| {
                            if value.has_type("Equipment") {
                                match recipient {
                                    AttachmentRecipient::Player(_) => true,
                                    AttachmentRecipient::Object(target_id) => {
                                        !super::targeting::equipment_attachment_legal(
                                            self, eq.id, target_id,
                                        )
                                    }
                                }
                            } else {
                                !value.is_aura() && !value.has_type("Fortification")
                            }
                        })
                    })
            })
            .map(|(id, _)| *id)
            .collect();
        for eq_id in attachments_to_unattach {
            if let Some(eq) = self.state.objects.get_mut(&eq_id) {
                eq.attached_to = None;
                changed = true;
                performed = true;
            }
        }

        // CR 111.7/111.8: tokens that have left the battlefield cease to exist.
        // CR 722.3c exempts the linked preparation copy in exile, not a spent or displaced copy.
        let spent_copies: Vec<_> = self
            .state
            .prepare_spell_sources
            .keys()
            .copied()
            .filter(|id| {
                let linked_exile_copy = self
                    .state
                    .prepared_permanents
                    .values()
                    .any(|copy| copy == id)
                    && self
                        .state
                        .objects
                        .get(id)
                        .is_some_and(|copy| copy.zone == Zone::Exile)
                    && self
                        .state
                        .zone_change_generation
                        .get(id)
                        .copied()
                        .unwrap_or(0)
                        == 0;
                let on_stack = self.state.stack.iter().any(|item| item.id == *id);
                !(on_stack || linked_exile_copy)
            })
            .collect();
        for id in spent_copies {
            self.state.prepare_spell_sources.remove(&id);
            self.state.zone_change_generation.remove(&id);
            self.state.class_levels.remove(&id);
            self.state.objects.remove(&id);
            self.state
                .active_exile_play_permissions
                .retain(|permission| permission.object_id != id);
            for player in &mut self.state.players {
                player.hand.retain(|&oid| oid != id);
                player.library.retain(|&oid| oid != id);
                player.exile.retain(|&oid| oid != id);
                player.graveyard.retain(|&oid| oid != id);
            }
            changed = true;
            performed = true;
        }
        let vanished: Vec<ObjectId> = self
            .state
            .objects
            .iter()
            .filter(|(_, o)| o.zone != Zone::Battlefield && o.is_token())
            .map(|(id, _)| *id)
            .collect();
        for id in vanished {
            if self.state.objects.remove(&id).is_some() {
                self.state.continuous_effects.retain(|effect| {
                    effect.source_id != Some(id)
                        || effect.duration != EffectDuration::WhileSourceInGraveyard
                });
                changed = true;
                performed = true;
                // Sweep every player, not just the owner: the battlefield list is keyed by
                // controller, so a token that changed control would otherwise leave a dangling
                // oid behind.
                for p in &mut self.state.players {
                    p.hand.retain(|&x| x != id);
                    p.battlefield.retain(|&x| x != id);
                    p.graveyard.retain(|&x| x != id);
                    p.exile.retain(|&x| x != id);
                    p.library.retain(|&x| x != id);
                }
            }
        }

        // CR 704.5m: an aura that is on the battlefield but not attached to a valid permanent is
        // put into its owner's graveyard. Checked after creature deaths so that the enchanted
        // permanent dying triggers this SBA on the re-check (CR 704.4).
        let orphaned_auras: Vec<ObjectId> = self
            .state
            .objects
            .iter()
            .filter(|(_, o)| {
                o.zone == Zone::Battlefield
                    && self
                        .characteristics(o.id)
                        .is_some_and(|value| value.is_aura())
                    && o.attached_to.is_none_or(|recipient| {
                        let enchant_filter = self.effective_face(o.id).and_then(|face| {
                            face.spell_effect.iter().find_map(|effect| match effect {
                                SpellEffectKind::AuraAttach { target } => Some(target.clone()),
                                _ => None,
                            })
                        });
                        enchant_filter.is_none_or(|filter| {
                            !super::targeting::attachment_filter_legal(
                                self,
                                &filter,
                                recipient,
                                o.id,
                                o.controller,
                            )
                        })
                    })
            })
            .map(|(id, _)| *id)
            .collect();
        let aura_zones = self.snapshot_zone_event();
        let mut aura_events = Vec::new();
        for id in orphaned_auras {
            let owner = self.state.objects.get(&id).map(|o| o.owner);
            let snapshot = aura_zones.source(id);
            let was_creature = self
                .characteristics(id)
                .is_some_and(|value| value.is_creature());
            if let Ok(died) = destroy_permanent(&mut self.state, self.registry, id) {
                changed = true;
                performed = true;
                if let Some(owner_id) = owner {
                    out.push(permanent_moved_event(
                        &self.state,
                        id,
                        owner_id,
                        rv1::permanent_moved::Destination::Graveyard,
                    ));
                }
                if let Some(source) = snapshot {
                    aura_events.extend(leaves_and_dies_events(source, was_creature, died));
                }
            }
        }

        self.fire_zone_triggers(aura_zones, aura_events, out);
        Ok((changed, performed))
    }

    pub(in crate::engine) fn finish_state_based_action_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let mut choices = match &pending.continuation {
            ResolutionContinuation::StateBasedActions { choices } => choices.clone(),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal(
                    "state-based action choice continuation missing",
                ));
            }
        };
        let Some(choice) = choices.choices.get(choices.next_choice_index).cloned() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("state-based action choice is stale"));
        };
        if pending.deciding_player != choice.deciding_player()
            || self.collect_state_based_action_choices() != choices.choices
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("state-based action choice is stale"));
        }

        let empty_extra_fields = answer.chosen_player_ids.is_empty()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty()
            && answer.cast_spell.is_none()
            && answer.spell_cast_announcement.is_none()
            && answer.chosen_combat_defender.is_none();
        let selected_answer = match choice {
            PendingStateBasedActionChoice::CommanderZone { .. }
                if decision == rv1::ResolutionChoiceDecision::SelectBranch
                    && answer.selected_branch_index <= 1
                    && answer.chosen_object_ids.is_empty()
                    && empty_extra_fields =>
            {
                Some(PendingStateBasedActionAnswer::CommanderZone {
                    move_to_command_zone: answer.selected_branch_index == 0,
                })
            }
            PendingStateBasedActionChoice::LegendKeep { candidates, .. }
                if decision == rv1::ResolutionChoiceDecision::Unspecified
                    && answer.selected_branch_index == 0
                    && answer.chosen_object_ids.len() == 1
                    && candidates
                        .iter()
                        .any(|(candidate, _)| *candidate == answer.chosen_object_ids[0])
                    && empty_extra_fields =>
            {
                Some(PendingStateBasedActionAnswer::LegendKeep {
                    object_id: answer.chosen_object_ids[0],
                })
            }
            _ => None,
        };
        let Some(selected_answer) = selected_answer else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("invalid state-based action choice"));
        };
        choices.answers.push(selected_answer);
        choices.next_choice_index += 1;

        let mut events = Vec::new();
        if choices.next_choice_index < choices.choices.len() {
            self.publish_state_based_action_choice(choices, &mut events)?;
            return Ok(super::events::finish_with_events(self, events));
        }
        if !self.state_based_action_choices_are_current(&choices) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("state-based action choice is stale"));
        }
        if let Some(index) = self.state.player_idx(self.state.active_player_id()) {
            self.state.priority_idx = index;
        }
        self.apply_sbas_after_choices(&mut events, choices)?;
        if self.state.pending_resolution.is_none() {
            events.push(super::events::ev_priority_changed(self));
        }
        Ok(super::events::finish_with_events(self, events))
    }

    /// Resolve every choice that applies in one CR 704.3 check before mutating that check's state.
    fn collect_state_based_action_choices(&self) -> Vec<PendingStateBasedActionChoice> {
        let mut choices = Vec::new();
        for player in &self.state.players {
            if player.has_lost {
                continue;
            }
            for &object_id in &player.declared_commander_object_ids {
                let Some(object) = self.state.objects.get(&object_id) else {
                    continue;
                };
                if !matches!(object.zone, Zone::Exile | Zone::Graveyard) {
                    continue;
                }
                let generation = self
                    .state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0);
                if self
                    .state
                    .commander_sba_checked_generations
                    .get(&object_id)
                    .copied()
                    == Some(generation)
                {
                    continue;
                }
                choices.push(PendingStateBasedActionChoice::CommanderZone {
                    object_id,
                    owner: object.owner,
                    source_zone: object.zone,
                    zone_change_generation: generation,
                });
            }
        }

        let mut legends: BTreeMap<(PlayerId, String), Vec<(ObjectId, u64)>> = BTreeMap::new();
        for (&object_id, object) in &self.state.objects {
            if object.zone != Zone::Battlefield {
                continue;
            }
            let Some(characteristics) = self.characteristics(object_id) else {
                continue;
            };
            if !characteristics.is_legendary() {
                continue;
            }
            let Some(name) = characteristics.primary_name().map(str::to_string) else {
                continue;
            };
            let generation = self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0);
            legends
                .entry((characteristics.controller, name))
                .or_default()
                .push((object_id, generation));
        }
        for ((controller, name), mut candidates) in legends {
            if candidates.len() < 2
                || self
                    .state
                    .player_idx(controller)
                    .is_none_or(|index| self.state.players[index].has_lost)
            {
                continue;
            }
            candidates.sort_unstable_by_key(|(object_id, _)| *object_id);
            choices.push(PendingStateBasedActionChoice::LegendKeep {
                controller,
                name,
                candidates,
            });
        }
        choices.sort_by_key(|choice| self.state.apnap_rank(choice.deciding_player()));
        choices
    }

    fn state_based_action_choices_are_current(
        &self,
        choices: &PendingStateBasedActionChoices,
    ) -> bool {
        if choices.next_choice_index != choices.choices.len()
            || choices.answers.len() != choices.choices.len()
            || self.collect_state_based_action_choices() != choices.choices
        {
            return false;
        }
        choices
            .choices
            .iter()
            .zip(&choices.answers)
            .all(|(choice, answer)| match (choice, answer) {
                (
                    PendingStateBasedActionChoice::CommanderZone { .. },
                    PendingStateBasedActionAnswer::CommanderZone { .. },
                ) => true,
                (
                    PendingStateBasedActionChoice::LegendKeep { candidates, .. },
                    PendingStateBasedActionAnswer::LegendKeep { object_id },
                ) => candidates
                    .iter()
                    .any(|(candidate, _)| candidate == object_id),
                _ => false,
            })
    }

    /// Present the next APNAP decision without applying any action from the frozen SBA set.
    pub(in crate::engine) fn publish_state_based_action_choice(
        &mut self,
        choices: PendingStateBasedActionChoices,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(choice) = choices.choices.get(choices.next_choice_index).cloned() else {
            return Err(EngineError::Illegal(
                "state-based action choice queue is empty",
            ));
        };
        let (presentation, event, prompt) = match &choice {
            PendingStateBasedActionChoice::CommanderZone {
                object_id,
                owner,
                source_zone,
                ..
            } => {
                let card_name = self
                    .state
                    .objects
                    .get(object_id)
                    .and_then(|object| self.registry.get(&object.card_id))
                    .map(|definition| definition.name.as_str())
                    .unwrap_or("this commander");
                let zone_name = match source_zone {
                    Zone::Exile => "Exile",
                    Zone::Graveyard => "their graveyard",
                    _ => "its current zone",
                };
                let prompt =
                    format!("P{owner}: put {card_name} into the command zone? (CR 704.6d/903.9a)");
                let branches = vec![
                    rv1::ResolutionBranchOption {
                        branch_index: 0,
                        label: "Put it into the command zone".into(),
                        selectable: true,
                        ..Default::default()
                    },
                    rv1::ResolutionBranchOption {
                        branch_index: 1,
                        label: format!("Leave it in {zone_name}"),
                        selectable: true,
                        ..Default::default()
                    },
                ];
                let event = rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                        rv1::ResolutionChoiceRequired {
                            deciding_player_id: *owner,
                            source_object_id: *object_id,
                            prompt_text: prompt.clone(),
                            choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                            min: 1,
                            max: 1,
                            resolution_branches: branches,
                            ..Default::default()
                        },
                    )),
                };
                let presentation = PendingResolutionPresentation {
                    source_object_id: *object_id,
                    candidates: Vec::new(),
                    min: 1,
                    max: 1,
                    ordered: false,
                    prompt: prompt.clone(),
                    choice_kind: custom::ChoiceKind::ResolutionBranch,
                    unique_names: false,
                };
                (presentation, event, prompt)
            }
            PendingStateBasedActionChoice::LegendKeep {
                controller,
                name,
                candidates,
            } => {
                let candidate_ids = candidates
                    .iter()
                    .map(|(object_id, _)| *object_id)
                    .collect::<Vec<_>>();
                let candidate_card_ids = candidates
                    .iter()
                    .map(|(object_id, _)| {
                        self.state
                            .objects
                            .get(object_id)
                            .map(|object| object.card_id.clone())
                            .unwrap_or_default()
                    })
                    .collect();
                let prompt = format!("Legend rule: choose which {name} to keep (CR 704.5j)");
                let event = rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                        rv1::ResolutionChoiceRequired {
                            variable_mana_contribution: false,
                            candidate_token_identities: Vec::new(),
                            candidate_player_ids: Vec::new(),
                            deciding_player_id: *controller,
                            source_object_id: candidate_ids[0],
                            prompt_text: prompt.clone(),
                            choice_kind: custom::ChoiceKind::LegendKeep as i32,
                            candidate_object_ids: candidate_ids.clone(),
                            candidate_card_ids,
                            candidate_names: vec![name.clone(); candidates.len()],
                            min: 1,
                            max: 1,
                            ordered: false,
                            unique_names: false,
                            candidate_server_card_ids: vec![],
                            candidate_selectable: Vec::new(),
                            resolution_branches: Vec::new(),
                            mana_cost: String::new(),
                            generic_mana_cost: 0,
                            payment_currently_legal: false,
                            public_reveal: None,
                            candidate_source_zones: Vec::new(),
                            combat_defender_options: Vec::new(),
                            waterbend: false,
                            selection_slots: Vec::new(),
                            replacement_options: Vec::new(),
                            selection_alternatives: Vec::new(),
                        },
                    )),
                };
                let presentation = PendingResolutionPresentation {
                    source_object_id: candidate_ids[0],
                    candidates: candidate_ids,
                    min: 1,
                    max: 1,
                    ordered: false,
                    prompt: prompt.clone(),
                    choice_kind: custom::ChoiceKind::LegendKeep,
                    unique_names: false,
                };
                (presentation, event, prompt)
            }
        };
        let deciding_player = choice.deciding_player();
        out.push(event);
        out.push(ev_log(prompt));
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player,
            presentation,
            continuation: ResolutionContinuation::StateBasedActions { choices },
        });
        Ok(())
    }

    /// Apply the first frozen SBA set after its choices, then continue checking until stable or
    /// another decision is required.
    pub(in crate::engine) fn apply_sbas_after_choices(
        &mut self,
        out: &mut Vec<rv1::RuledEvent>,
        choices: PendingStateBasedActionChoices,
    ) -> Result<bool, EngineError> {
        if self.state.is_terminal() {
            return Ok(false);
        }
        let mut performed = false;
        let (mut changed, performed_this_pass) =
            self.apply_sbas_once_with_choices(out, Some(choices))?;
        performed |= performed_this_pass;
        while !self.state.is_terminal() && self.state.pending_resolution.is_none() {
            if !changed {
                break;
            }
            let (next_changed, performed_this_pass) = self.apply_sbas_once(out)?;
            changed = next_changed;
            performed |= performed_this_pass;
        }
        self.debug_assert_battlefield_control_index();
        Ok(performed)
    }

    /// Materialize CR 613 layer-2 control into the battlefield control index. Characteristics
    /// remain the authority; `GameObject::controller` is the settled cache used by hot paths.
    pub(super) fn reindex_battlefield_control(&mut self, out: &mut Vec<rv1::RuledEvent>) -> bool {
        let mut ordered = Vec::new();
        for player in &self.state.players {
            for &oid in &player.battlefield {
                if !ordered.contains(&oid) {
                    ordered.push(oid);
                }
            }
        }
        let mut missing: Vec<_> = self
            .state
            .objects
            .iter()
            .filter(|(oid, object)| object.zone == Zone::Battlefield && !ordered.contains(oid))
            .map(|(oid, _)| *oid)
            .collect();
        missing.sort_unstable();
        ordered.extend(missing);

        let mut expired_source_control_duration = false;
        let desired = loop {
            let desired: Vec<_> = ordered
                .iter()
                .filter_map(|&oid| {
                    self.state
                        .objects
                        .get(&oid)
                        .filter(|object| object.zone == Zone::Battlefield)
                        .and_then(|_| {
                            self.characteristics(oid)
                                .map(|value| (oid, value.controller))
                        })
                })
                .collect();
            let expired: Vec<_> = self
                .state
                .continuous_effects
                .iter()
                .filter_map(|effect| {
                    let EffectDuration::WhileSourceControlledBy {
                        source_object_id,
                        source_zone_change_generation,
                        controller,
                    } = &effect.duration
                    else {
                        return None;
                    };
                    let source_is_current = self
                        .state
                        .objects
                        .get(source_object_id)
                        .is_some_and(|source| source.zone == Zone::Battlefield)
                        && self
                            .state
                            .zone_change_generation
                            .get(source_object_id)
                            .copied()
                            .unwrap_or(0)
                            == *source_zone_change_generation;
                    let source_controller = desired.iter().find_map(|&(object_id, current)| {
                        (object_id == *source_object_id).then_some(current)
                    });
                    let ability_controller_is_in_game = self
                        .state
                        .player_idx(*controller)
                        .is_some_and(|index| !self.state.players[index].has_lost);
                    (!source_is_current
                        || source_controller != Some(*controller)
                        || !ability_controller_is_in_game)
                        .then_some((
                            *source_object_id,
                            *source_zone_change_generation,
                            *controller,
                        ))
                })
                .collect();
            if expired.is_empty() {
                break desired;
            }

            // A "for as long as" control duration ends permanently on its first loss. Remove
            // every duration that is false in this layer-2 snapshot, then recompute. No controller
            // cache or observer sees an intermediate state from an earlier pass.
            self.state.continuous_effects.retain(|effect| {
                !matches!(
                    &effect.duration,
                    EffectDuration::WhileSourceControlledBy {
                        source_object_id,
                        source_zone_change_generation,
                        controller,
                    } if expired.contains(&(
                        *source_object_id,
                        *source_zone_change_generation,
                        *controller,
                    ))
                )
            });
            expired_source_control_duration = true;
        };
        let mut changed_ids = Vec::new();
        let mut control_transitions = Vec::new();
        for &(oid, controller) in &desired {
            if self.state.player_idx(controller).is_none() {
                continue;
            }
            if let Some(object) = self.state.objects.get_mut(&oid) {
                if object.controller != controller {
                    control_transitions.push((oid, object.controller, Some(controller)));
                    object.controller = controller;
                    object.summoning_sick = true;
                    changed_ids.push(oid);
                }
            }
        }
        if changed_ids.is_empty() {
            self.reconcile_combat_characteristics(out);
            return expired_source_control_duration;
        }

        for player in &mut self.state.players {
            player.battlefield.clear();
        }
        for (oid, controller) in desired {
            if let Some(index) = self.state.player_idx(controller) {
                self.state.players[index].battlefield.push(oid);
            }
        }

        self.remove_combat_participants(&changed_ids, out);
        self.reconcile_combat_characteristics(out);
        for (object_id, old_controller, new_controller) in control_transitions {
            let object = TriggerObjectRef {
                object_id,
                zone_change_generation: self
                    .state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0),
                controller_at_event: old_controller,
            };
            let delayed =
                self.state
                    .dispatch_event_observers(ObservedGameEvent::ControllerChanged {
                        object,
                        old_controller,
                        new_controller,
                    });
            self.state.stage_delayed_batch(delayed);
        }
        true
    }
}

#[cfg(test)]
mod sba_tests {
    use super::super::resolution::move_object_to_zone;
    use super::*;

    fn engine() -> GameEngine {
        GameEngine::new_with_default_decks(tricerules_cards::registry::global(), 1, &[0, 1], 20)
            .expect("new")
    }

    fn engine_with_exiled_commander() -> (GameEngine, ObjectId) {
        let decks = vec![
            EngineDeck {
                mainboard: vec!["mountain".into(); 12],
                commanders: vec!["kami_of_the_crescent_moon".into()],
            },
            EngineDeck {
                mainboard: vec!["island".into(); 12],
                commanders: Vec::new(),
            },
        ];
        let mut engine = GameEngine::new_with_commander_decks(
            tricerules_cards::registry::global(),
            1,
            &[0, 1],
            20,
            Some(decks),
            true,
        )
        .expect("Commander game");
        let commander = engine.state.players[0].command_zone[0];
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            commander,
            Zone::Exile,
            None,
        )
        .expect("put declared commander in Exile");
        (engine, commander)
    }

    fn add_registered_permanent(
        engine: &mut GameEngine,
        owner: PlayerId,
        card_id: &str,
    ) -> ObjectId {
        let id = engine.state.next_object_id;
        engine.state.next_object_id += 1;
        let face = engine
            .registry
            .get(card_id)
            .unwrap_or_else(|| panic!("registered test card {card_id}"))
            .primary_face();
        let object =
            super::super::new_object_from_card(id, owner, card_id, Zone::Battlefield, face);
        engine.state.objects.insert(id, object);
        let index = engine.state.player_idx(owner).expect("owner is seated");
        engine.state.players[index].battlefield.push(id);
        id
    }

    #[test]
    fn commander_sba_choice_freezes_coexisting_death_and_legend_actions() {
        use tricerules_proto::ruled::v1 as rv1;

        let (mut engine, commander) = engine_with_exiled_commander();
        let zero_toughness = add_creature(&mut engine, 1, 0, 0);
        let first_legend = add_registered_permanent(&mut engine, 1, "kokusho,_the_evening_star");
        let second_legend = add_registered_permanent(&mut engine, 1, "kokusho,_the_evening_star");
        let first_legend_generation = engine
            .state
            .zone_change_generation
            .get(&first_legend)
            .copied()
            .unwrap_or(0);
        let mut events = Vec::new();
        engine
            .apply_sbas(&mut events)
            .expect("state-based actions are checked");

        let commander_choice = events.iter().find_map(|event| match &event.ev {
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                if choice.prompt_text.contains("command zone") =>
            {
                Some(choice)
            }
            _ => None,
        });
        assert!(
            commander_choice.is_some(),
            "CR 903.9a offers the owner a command-zone choice during the frozen SBA check"
        );
        assert_eq!(
            engine.state.objects[&zero_toughness].zone,
            Zone::Battlefield,
            "mandatory SBAs wait until the command-zone decision is collected"
        );
        assert_eq!(engine.state.objects[&first_legend].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&second_legend].zone, Zone::Battlefield);
        // Model the already-published view accompanying the pending choice. The next answer may
        // send a routine unchanged snapshot, but must not reveal a partial SBA action set.
        let pending_view = engine.ev_zone_view_sync_tracked();
        let Some(rv1::ruled_event::Ev::ZoneView(pending_view)) = pending_view.ev else {
            panic!("the pending SBA choice has a zone-view snapshot");
        };
        let public_zone_state = |view: &rv1::ZoneViewSync| {
            view.per_player
                .iter()
                .map(|player| {
                    (
                        player.player_id,
                        player.battlefield_objects.clone(),
                        player.graveyard_object_ids.clone(),
                        player.exile_object_ids.clone(),
                        player.command_zone_object_ids.clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let pending_public_state = public_zone_state(&pending_view);

        let first_choice = engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                            selected_branch_index: 0,
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("accept command-zone move");
        assert_eq!(engine.state.objects[&commander].zone, Zone::Exile);
        assert_eq!(
            engine.state.objects[&zero_toughness].zone,
            Zone::Battlefield
        );
        assert_eq!(engine.state.objects[&first_legend].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&second_legend].zone, Zone::Battlefield);
        let intermediate_views = first_choice
            .events
            .iter()
            .filter_map(|event| match event.ev.as_ref() {
                Some(rv1::ruled_event::Ev::ZoneView(view)) => Some(view),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            !first_choice.events.iter().any(|event| matches!(
                event.ev.as_ref(),
                Some(rv1::ruled_event::Ev::PriorityChanged(_))
            )) && intermediate_views.len() == 1
                && public_zone_state(intermediate_views[0]) == pending_public_state,
            "the next prompt may publish a routine view, but no SBA action or priority is visible before all choices are answered"
        );
        let next = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("legend choice follows the Commander choice");
        assert_eq!(
            next.deciding_player, 1,
            "APNAP orders the nonactive controller next"
        );
        assert_eq!(
            next.presentation.choice_kind,
            custom::ChoiceKind::LegendKeep
        );

        let frozen_generation = engine
            .state
            .zone_change_generation
            .get(&commander)
            .copied()
            .unwrap_or(0);
        assert_eq!(
            engine
                .state
                .commander_sba_checked_generations
                .get(&commander),
            None,
            "decisions do not commit the generation ledger before the full set is answered"
        );
        assert_eq!(frozen_generation, 1);

        let final_choice = engine
            .apply_command(
                1,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![second_legend],
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("finish the frozen SBA choice set");

        assert_eq!(engine.state.objects[&commander].zone, Zone::Command);
        assert_eq!(engine.state.objects[&zero_toughness].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&first_legend].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&second_legend].zone, Zone::Battlefield);
        assert_eq!(
            engine
                .state
                .commander_sba_checked_generations
                .get(&commander),
            Some(&1)
        );
        let death_trigger = engine
            .state
            .stack
            .last()
            .expect("the removed Kokusho trigger uses its pre-action LKI");
        assert_eq!(death_trigger.card_id, "kokusho,_the_evening_star");
        assert_eq!(death_trigger.source_permanent_id, Some(first_legend));
        assert_eq!(death_trigger.source_zone_change, first_legend_generation);
        assert_eq!(
            final_choice
                .events
                .iter()
                .filter(|event| {
                    matches!(event.ev.as_ref(), Some(rv1::ruled_event::Ev::ZoneView(_)))
                })
                .count(),
            1,
            "one final view publishes the committed state; no intermediate view escapes"
        );
    }

    #[test]
    fn commander_move_from_graveyard_dispatches_card_departure_trigger() {
        use tricerules_proto::ruled::v1 as rv1;

        let (mut engine, commander) = engine_with_exiled_commander();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            commander,
            Zone::Graveyard,
            None,
        )
        .expect("put the declared commander in its owner's graveyard");
        let mascot = add_registered_permanent(&mut engine, 0, "spirit_mascot");

        let mut initial_events = Vec::new();
        engine
            .apply_sbas(&mut initial_events)
            .expect("the graveyard commander receives its CR 903.9a choice");
        assert!(engine.state.pending_resolution.is_some());
        engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                            selected_branch_index: 0,
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("accept the command-zone move");

        assert_eq!(engine.state.objects[&commander].zone, Zone::Command);
        assert!(
            engine
                .state
                .stack
                .iter()
                .any(|item| item.source_permanent_id == Some(mascot)),
            "Spirit Mascot observes the commander leaving its controller's graveyard"
        );
    }

    #[test]
    fn declined_graveyard_commander_choice_does_not_dispatch_a_departure_trigger() {
        use tricerules_proto::ruled::v1 as rv1;

        let (mut engine, commander) = engine_with_exiled_commander();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            commander,
            Zone::Graveyard,
            None,
        )
        .expect("put the declared commander in its owner's graveyard");
        let mascot = add_registered_permanent(&mut engine, 0, "spirit_mascot");

        let mut initial_events = Vec::new();
        engine
            .apply_sbas(&mut initial_events)
            .expect("the graveyard commander receives its CR 903.9a choice");
        engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                            selected_branch_index: 1,
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("decline the command-zone move");

        assert_eq!(engine.state.objects[&commander].zone, Zone::Graveyard);
        assert!(
            engine
                .state
                .stack
                .iter()
                .all(|item| item.source_permanent_id != Some(mascot)),
            "Spirit Mascot does not trigger when the commander remains in the graveyard"
        );
    }

    #[test]
    fn simultaneous_deaths_preserve_source_power_for_a_pending_trigger() {
        use tricerules_proto::ruled::v1 as rv1;

        let mut engine = engine();
        let anthem = add_registered_permanent(&mut engine, 0, "captain_of_the_watch");
        engine.emit_static_abilities_on_enter(anthem);
        let source = add_registered_permanent(&mut engine, 0, "brambleguard_captain");
        let target = add_creature(&mut engine, 0, 2, 0);
        assert!(anthem < source, "the anthem leaves first by object id");
        assert_eq!(
            engine.characteristics(source).unwrap().signed_power,
            Some(3)
        );

        let mut trigger_events = Vec::new();
        engine.fire_triggers(
            &[GameEvent::PhaseBegan {
                phase: rv1::PhaseId::BeginCombat,
                active_player: 0,
            }],
            &mut trigger_events,
        );
        engine.flush_staged_triggers(&mut trigger_events);
        assert_eq!(
            engine.state.pending_triggers.front().unwrap().card_id,
            "brambleguard_captain"
        );
        engine
            .choose_trigger_target(
                0,
                &[rv1::TargetRef {
                    object_id: target,
                    ..Default::default()
                }],
                &[],
                false,
            )
            .expect("put the pending Brambleguard trigger on the stack");

        engine.state.objects.get_mut(&anthem).unwrap().damage = 3;
        engine.state.objects.get_mut(&source).unwrap().damage = 4;
        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut sba_events = Vec::new();
        engine
            .apply_sbas(&mut sba_events)
            .expect("perform simultaneous deaths");

        assert_eq!(engine.state.objects[&anthem].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        assert_eq!(
            engine
                .state
                .last_known_pt_by_generation
                .get(&(source, source_generation)),
            Some(&(Some(3), Some(4))),
            "the source leaves with the 3/4 characteristics it had before the anthem left"
        );

        let mut resolution_events = Vec::new();
        engine
            .resolve_top_of_stack(&mut resolution_events)
            .expect("resolve the trigger using source LKI");
        assert_eq!(engine.effective_power(target), Some(5));
    }

    #[test]
    fn saga_sacrifice_uses_pre_sba_source_snapshot_after_granting_source_leaves() {
        let mut engine = engine();
        engine.state.opening = None;
        let granting_source = add_creature(&mut engine, 0, 2, 0);
        let saga = add_registered_permanent(&mut engine, 0, "burn,_burn,_tree_and_fern");
        let observer = add_registered_permanent(&mut engine, 1, "blood_artist");
        engine
            .state
            .objects
            .get_mut(&saga)
            .expect("Saga")
            .set_counter(CounterKind::Lore, 4);
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: Some(granting_source),
            affected: AffectedScope::Single(saga),
            kind: ContinuousEffectKind::Layer4AddTypes(tricerules_card_model::TypeLineAddition {
                card_types: vec![PermanentTypeFilter::Creature],
                ..tricerules_card_model::TypeLineAddition::default()
            }),
            condition: None,
            duration: EffectDuration::WhileSourceOnBattlefield,
            timestamp: engine.state.command_index,
        });
        let granted_saga_dies = tricerules_card_model::TriggeredAbilityDef {
            ability_id: tricerules_card_model::AbilityId::new("granted_saga_dies")
                .expect("test ability id"),
            presentation: tricerules_card_model::AbilityPresentation::Fallback,
            trigger: tricerules_card_model::TriggerCondition::WhenSelfDies,
            effect: vec![tricerules_card_model::SpellEffectKind::GainLife {
                amount: tricerules_card_model::Amount::Fixed(1),
            }],
            modal: None,
            targeting: None,
            may: false,
            intervening_if: None,
            triggers_only_once: false,
            max_triggers_per_turn: None,
        };
        engine.state.add_triggered_ability_grant(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: Some(granting_source),
            affected: AffectedScope::Single(saga),
            kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(granted_saga_dies)),
            condition: None,
            duration: EffectDuration::WhileSourceOnBattlefield,
            timestamp: engine.state.command_index,
        });
        assert!(engine.characteristics(saga).unwrap().is_creature());
        assert_eq!(
            engine
                .trigger_source_snapshot(observer)
                .unwrap()
                .triggered_abilities
                .len(),
            1,
            "the test observer has its printed dies-observer ability before the SBA"
        );
        assert!(engine
            .characteristics(granting_source)
            .unwrap()
            .is_creature());

        engine
            .state
            .objects
            .get_mut(&granting_source)
            .expect("granting creature")
            .damage = 2;
        let mut events = Vec::new();
        engine
            .apply_sbas(&mut events)
            .expect("the creature and final-chapter Saga leave in one SBA set");

        assert_eq!(engine.state.objects[&granting_source].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&saga].zone, Zone::Graveyard);
        assert!(
            engine
                .state
                .staged_trigger_groups
                .iter()
                .flat_map(|group| &group.triggers)
                .any(|trigger| {
                    trigger.source_permanent_id == saga
                        && trigger.ability.ability_id.as_str() == "granted_saga_dies"
                }),
            "the Saga's LTB event uses its trigger snapshot from before its grantor left"
        );
        assert!(!engine
            .trigger_source_snapshot(saga)
            .unwrap()
            .types
            .iter()
            .any(|kind| kind == "Creature"));
        let observer_triggers = engine
            .state
            .staged_trigger_groups
            .iter()
            .flat_map(|group| &group.triggers)
            .filter(|trigger| trigger.source_permanent_id == observer)
            .count()
            + engine
                .state
                .pending_triggers
                .iter()
                .filter(|trigger| trigger.source_permanent_id == observer)
                .count()
            + engine
                .state
                .stack
                .iter()
                .filter(|trigger| trigger.source_permanent_id == Some(observer))
                .count();
        assert_eq!(
            observer_triggers,
            2,
            "Blood Artist sees both creatures die, including the Saga whose type came from the departing source; blocking={:?}, staged={}, pending={}",
            engine.state.blocking_choice(),
            engine.state.staged_trigger_groups.iter().map(|group| group.triggers.len()).sum::<usize>(),
            engine.state.pending_triggers.len(),
        );
    }

    #[test]
    fn exile_departures_are_retained_in_zone_event_snapshots() {
        let (mut engine, commander) = engine_with_exiled_commander();
        let snapshot = engine.snapshot_zone_event();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            commander,
            Zone::Command,
            None,
        )
        .expect("move the commander to its command zone");

        let GameEvent::ZoneChanges(batch) = engine.finish_zone_event(snapshot) else {
            panic!("zone snapshot finishes as a zone-change batch");
        };
        let movement = batch
            .moves
            .iter()
            .find(|movement| movement.before.object_id == commander)
            .expect("Exile-origin object is present in the zone-change receipt");
        assert_eq!(movement.origin, Zone::Exile);
        assert_eq!(movement.destination, Zone::Command);
        assert!(movement.before.types.iter().any(|kind| kind == "Creature"));
    }

    #[test]
    fn face_down_exile_departures_do_not_reveal_card_characteristics() {
        let (mut engine, commander) = engine_with_exiled_commander();
        engine.state.objects.get_mut(&commander).unwrap().face_down = true;
        let snapshot = engine.snapshot_zone_event();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            commander,
            Zone::Command,
            None,
        )
        .expect("move the face-down commander to its command zone");

        let GameEvent::ZoneChanges(batch) = engine.finish_zone_event(snapshot) else {
            panic!("zone snapshot finishes as a zone-change batch");
        };
        let movement = batch
            .moves
            .iter()
            .find(|movement| movement.before.object_id == commander)
            .expect("face-down Exile-origin object remains in the receipt");
        assert!(movement.before.types.is_empty());
        assert!(!movement.before.all_creature_types);
        assert!(movement.before.keywords.is_empty());
        assert_eq!(movement.before.power, None);
    }

    #[test]
    fn declined_commander_sba_choice_is_reoffered_after_a_zone_change() {
        use tricerules_proto::ruled::v1 as rv1;

        let (mut engine, commander) = engine_with_exiled_commander();
        let mut initial_events = Vec::new();
        engine
            .apply_sbas(&mut initial_events)
            .expect("state-based actions are checked");
        let first_generation = engine
            .state
            .zone_change_generation
            .get(&commander)
            .copied()
            .unwrap_or(0);
        assert!(initial_events.iter().any(|event| matches!(
            event.ev.as_ref(),
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                if choice.prompt_text.contains("command zone")
        )));

        engine
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                            selected_branch_index: 1,
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("decline the command-zone move");
        assert_eq!(engine.state.objects[&commander].zone, Zone::Exile);
        assert_eq!(
            engine
                .state
                .commander_sba_checked_generations
                .get(&commander),
            Some(&first_generation)
        );

        let mut unchanged_events = Vec::new();
        engine
            .apply_sbas(&mut unchanged_events)
            .expect("the same zone-change generation does not prompt again");
        assert!(!unchanged_events.iter().any(|event| matches!(
            event.ev.as_ref(),
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                if choice.prompt_text.contains("command zone")
        )));

        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            commander,
            Zone::Graveyard,
            None,
        )
        .expect("move the declined commander to a different zone");
        let next_generation = engine
            .state
            .zone_change_generation
            .get(&commander)
            .copied()
            .unwrap_or(0);
        assert!(next_generation > first_generation);
        let mut reentry_events = Vec::new();
        engine
            .apply_sbas(&mut reentry_events)
            .expect("a new zone-change generation is offered again");
        assert!(reentry_events.iter().any(|event| matches!(
            event.ev.as_ref(),
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                if choice.prompt_text.contains("command zone")
        )));
        assert!(engine.state.pending_resolution.is_some());
    }

    /// Put a vanilla creature ("walking_corpse", no keywords/triggers, non-legendary, non-token)
    /// on `owner`'s battlefield with the given base toughness and marked damage; returns its id.
    fn add_creature(e: &mut GameEngine, owner: PlayerId, toughness: u32, damage: u32) -> ObjectId {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            GameObject {
                id,
                owner,
                base_controller: owner,
                controller: owner,
                card_id: "walking_corpse".to_string(),
                token_origin: None,
                token_faces: None,
                copiable_values: None,
                copy_revision: 0,
                active_copy_occurrence: None,
                zone: Zone::Battlefield,
                tapped: false,
                summoning_sick: false,
                power: Some(2),
                toughness: Some(toughness),
                damage,
                deathtouch_damage: false,
                counters: Default::default(),
                counter_timestamps: Default::default(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
                face_down: false,
            },
        );
        let idx = e.state.player_idx(owner).unwrap();
        e.state.players[idx].battlefield.push(id);
        id
    }

    #[test]
    fn basic_land_transformation_detaches_former_attachment_without_moving_it() {
        for original_type in ["Aura", "Equipment", "Fortification"] {
            let mut e = engine();
            e.state.opening = None;
            let recipient = add_creature(&mut e, 0, 2, 0);
            let attachment = add_creature(&mut e, 0, 2, 0);
            let base_card = if original_type == "Aura" {
                "hermetic_study"
            } else {
                "bonesplitter"
            };
            let mut face = e.registry.get(base_card).unwrap().primary_face().clone();
            face.types = vec![
                if original_type == "Aura" {
                    "Enchantment"
                } else {
                    "Artifact"
                }
                .into(),
                original_type.into(),
            ];
            let object = e.state.objects.get_mut(&attachment).unwrap();
            object.card_id = base_card.into();
            object.copiable_values = Some(CopiableValues {
                source_card_id: base_card.into(),
                source_face_index: 0,
                display_name: face.name.clone(),
                face,
                room_faces: None,
            });
            object.attached_to = Some(AttachmentRecipient::Object(recipient));
            // An Aura enchanting any permanent remains attached to the transformed attachment.
            let outer = add_creature(&mut e, 0, 2, 0);
            let mut outer_face = e
                .registry
                .get("hermetic_study")
                .unwrap()
                .primary_face()
                .clone();
            outer_face.spell_effect = vec![SpellEffectKind::AuraAttach {
                target: tricerules_card_model::primitives::TargetFilter {
                    kind: tricerules_card_model::primitives::TargetKind::AnyPermanent,
                    ..Default::default()
                },
            }];
            let object = e.state.objects.get_mut(&outer).unwrap();
            object.card_id = "hermetic_study".into();
            object.copiable_values = Some(CopiableValues {
                source_card_id: "hermetic_study".into(),
                source_face_index: 0,
                display_name: outer_face.name.clone(),
                face: outer_face,
                room_faces: None,
            });
            object.attached_to = Some(AttachmentRecipient::Object(attachment));
            e.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(attachment),
                kind: ContinuousEffectKind::Layer4SetTypeLine(
                    tricerules_card_model::TypeLineReplacement {
                        card_types: vec![PermanentTypeFilter::Land],
                        creature_types: Vec::new(),
                        land_types: vec![BasicLandType::Forest],
                    },
                ),
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 1,
            });
            e.apply_sbas(&mut Vec::new()).unwrap();
            assert_eq!(e.state.objects[&attachment].zone, Zone::Battlefield);
            assert_eq!(
                e.state.objects[&attachment].attached_to, None,
                "former {original_type} becomes unattached"
            );
            assert_eq!(e.state.objects[&outer].zone, Zone::Battlefield);
            assert_eq!(
                e.state.objects[&outer].attached_to,
                Some(AttachmentRecipient::Object(attachment))
            );
            let Some(rv1::ruled_event::Ev::ZoneView(view)) = e.ev_zone_view_sync().ev else {
                panic!("zone view");
            };
            let projected = view
                .per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == attachment)
                .unwrap();
            assert!(projected.attachment_recipient.is_none());
            e.state.continuous_effects.clear();
            e.apply_sbas(&mut Vec::new()).unwrap();
            assert_eq!(e.state.objects[&attachment].attached_to, None);
            assert_eq!(
                e.state.objects[&attachment].zone,
                if original_type == "Aura" {
                    Zone::Graveyard
                } else {
                    Zone::Battlefield
                }
            );
        }
    }

    #[test]
    fn poison_counters_cause_a_player_to_lose_at_ten() {
        // CR 704.5c: the loss is a state-based action at ten or more poison counters.
        let mut e = engine();
        e.state.players[0].counters.insert(CounterKind::Poison, 9);
        e.apply_sbas(&mut Vec::new())
            .expect("nine poison is not lethal");
        assert!(!e.state.players[0].has_lost);

        e.state.players[0].counters.insert(CounterKind::Poison, 10);
        e.apply_sbas(&mut Vec::new()).expect("ten poison is lethal");

        assert!(
            e.state.players[0].has_lost,
            "a player with ten poison counters loses as a state-based action"
        );
    }

    #[test]
    fn player_counter_counts_are_in_the_public_zone_view() {
        let mut e = engine();
        e.state.players[1].counters.insert(CounterKind::Poison, 3);

        let event = e.ev_zone_view_sync_tracked();
        let Some(rv1::ruled_event::Ev::ZoneView(view)) = event.ev else {
            panic!("expected a zone view");
        };
        let player = view
            .per_player
            .iter()
            .find(|player| player.player_id == 1)
            .expect("player view");
        assert_eq!(player.player_counters.len(), 1);
        assert_eq!(player.player_counters[0].name, "poison");
        assert_eq!(player.player_counters[0].count, 3);
    }

    #[test]
    fn issue_153_counter_annihilation_and_death_use_the_same_snapshot() {
        let mut e = engine();
        let creature = add_creature(&mut e, 0, 2, 0);
        let object = e.state.objects.get_mut(&creature).unwrap();
        object.add_counters(CounterKind::PlusOnePlusOne, 1, 0);
        object.add_counters(CounterKind::MinusOneMinusOne, 1, 0);
        e.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: Some(creature),
            affected: AffectedScope::Single(creature),
            kind: ContinuousEffectKind::PtModify {
                delta_power: 0,
                delta_toughness: -2,
            },
            condition: Some(
                tricerules_card_model::primitives::GameCondition::SourceCounterCount {
                    counter: CounterKind::PlusOnePlusOne,
                    min: Some(1),
                    max: None,
                },
            ),
            duration: EffectDuration::WhileSourceOnBattlefield,
            timestamp: 0,
        });
        assert_eq!(e.effective_toughness(creature), Some(0));
        e.apply_sbas_once(&mut Vec::new()).unwrap();
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Graveyard,
            "annihilation must not alter the simultaneous death decision"
        );
    }

    fn anthem(source: ObjectId, dt: i32, duration: EffectDuration) -> ContinuousEffect {
        ContinuousEffect {
            trigger_grant_origin: None,
            source_id: Some(source),
            affected: AffectedScope::AllCreatures,
            kind: ContinuousEffectKind::PtModify {
                delta_power: 0,
                delta_toughness: dt,
            },
            condition: None,
            duration,
            timestamp: 0,
        }
    }

    #[test]
    fn static_anthem_drained_when_source_leaves_battlefield() {
        // CR 611.3: a static-ability continuous effect ends the moment its source leaves play.
        let mut e = engine();
        let src = add_creature(&mut e, 0, 2, 0);
        let other = add_creature(&mut e, 0, 1, 0);
        e.state
            .continuous_effects
            .push(anthem(src, 1, EffectDuration::WhileSourceOnBattlefield));
        assert_eq!(e.effective_toughness(other), Some(2)); // base 1 + anthem +1
        move_object_to_zone(&mut e.state, e.registry, src, Zone::Graveyard, None).unwrap();
        assert_eq!(e.effective_toughness(other), Some(1)); // anthem gone
        assert!(e.state.continuous_effects.is_empty());
    }

    #[test]
    fn one_shot_pump_survives_source_leaving() {
        // CR 611.2g: a one-shot pump (e.g. firebreathing) is independent of its source once made,
        // so the source leaving must NOT drain it — only `WhileSourceOnBattlefield` effects drain.
        let mut e = engine();
        let src = add_creature(&mut e, 0, 2, 0);
        let target = add_creature(&mut e, 0, 1, 0);
        e.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: Some(src),
            affected: AffectedScope::Single(target),
            kind: ContinuousEffectKind::PtModify {
                delta_power: 0,
                delta_toughness: 2,
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 0,
        });
        move_object_to_zone(&mut e.state, e.registry, src, Zone::Graveyard, None).unwrap();
        assert_eq!(e.effective_toughness(target), Some(3)); // still buffed
    }

    #[test]
    fn continuous_control_reindexes_the_battlefield_and_marks_the_permanent_sick() {
        let mut e = engine();
        let target = add_creature(&mut e, 0, 2, 0);
        e.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(target),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(1),
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1,
        });

        let mut out = vec![];
        e.apply_sbas(&mut out).expect("state-based actions");

        let object = e.state.objects.get(&target).expect("target");
        assert_eq!(object.base_controller, 0);
        assert_eq!(object.controller, 1);
        assert!(object.summoning_sick);
        assert!(!e.state.players[0].battlefield.contains(&target));
        assert!(e.state.players[1].battlefield.contains(&target));
    }

    #[test]
    fn resolving_source_control_duration_expires_before_control_is_published() {
        let mut e = engine();
        let source = add_creature(&mut e, 0, 2, 0);
        let target = add_creature(&mut e, 1, 2, 0);
        let source_generation = e
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        e.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            // This is the stack ability's id, deliberately distinct from the permanent source.
            source_id: Some(u32::MAX),
            affected: AffectedScope::Single(target),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(0),
            },
            condition: None,
            duration: EffectDuration::WhileSourceControlledBy {
                source_object_id: source,
                source_zone_change_generation: source_generation,
                controller: 0,
            },
            timestamp: 1,
        });
        let mut out = Vec::new();
        e.reindex_battlefield_control(&mut out);
        assert_eq!(e.state.objects[&target].controller, 0);

        e.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(1),
            },
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: 2,
        });
        out.clear();
        e.reindex_battlefield_control(&mut out);

        assert_eq!(e.state.objects[&source].controller, 1);
        assert_eq!(
            e.state.objects[&target].controller, 1,
            "losing control of the exact source must end the earlier control lease"
        );
        assert_eq!(
            e.state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            source_generation,
            "source control changes do not create a new object"
        );
    }

    #[test]
    fn resolving_source_control_duration_drains_for_the_exact_source_generation() {
        let mut e = engine();
        let source = add_creature(&mut e, 0, 2, 0);
        let target = add_creature(&mut e, 1, 2, 0);
        let source_generation = e
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        e.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            // The creating stack ability id is not the battlefield source id.
            source_id: Some(u32::MAX),
            affected: AffectedScope::Single(target),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(0),
            },
            condition: None,
            duration: EffectDuration::WhileSourceControlledBy {
                source_object_id: source,
                source_zone_change_generation: source_generation,
                controller: 0,
            },
            timestamp: 1,
        });
        assert_eq!(e.characteristics(target).unwrap().controller, 0);

        move_object_to_zone(&mut e.state, e.registry, source, Zone::Graveyard, None)
            .expect("move the exact source incarnation");

        assert_eq!(
            e.state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            source_generation + 1
        );
        assert!(e.state.continuous_effects.is_empty());
        assert_eq!(e.characteristics(target).unwrap().controller, 1);
    }

    #[test]
    fn control_change_removes_the_permanent_from_combat() {
        let mut e = engine();
        let target = add_creature(&mut e, 0, 2, 0);
        e.state.combat = Some(CombatState {
            attacking: vec![target],
            attack_assignments: HashMap::new(),
            blockers: HashMap::new(),
            damage_assignments: HashMap::new(),
            trample_player_damage: HashMap::new(),
            damage_assignment_needed: false,
            attackers_declared: true,
            blockers_declared_by: Vec::new(),
            blockers_declared: false,
            assign_combat_damage_phase: false,
            first_strike_attackers: vec![],
            first_strike_blockers: HashMap::new(),
            first_strike_damage_done: false,
        });
        e.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(target),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(1),
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1,
        });

        let mut out = vec![];
        e.apply_sbas(&mut out).expect("state-based actions");

        assert!(!e
            .state
            .combat
            .as_ref()
            .expect("combat")
            .attacking
            .contains(&target));
        assert!(out.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::RemovedFromCombat(removed))
                if removed.object_ids == [target]
        )));
    }

    #[test]
    fn sba_cascades_to_fixpoint_when_anthem_dies() {
        // CR 704.4: SBAs re-check until stable. The anthem (+0/+1, AllCreatures, source-bound)
        // keeps `dependent` alive; the anthem's source has lethal damage and dies on the first
        // pass, draining the anthem; the *same* `apply_sbas` call must then catch `dependent`.
        let mut e = engine();
        // src: base toughness 1, damage 2. The anthem buffs src too -> eff toughness 2, damage 2,
        // so it dies on pass 1 (the anthem can't save its own source here).
        let src = add_creature(&mut e, 0, 1, 2);
        // dependent: base toughness 1, damage 1. With the +1 anthem it's eff toughness 2 (lives on
        // pass 1); once the anthem drains it's eff toughness 1 with 1 damage (dies on the re-check).
        let dependent = add_creature(&mut e, 0, 1, 1);
        e.state
            .continuous_effects
            .push(anthem(src, 1, EffectDuration::WhileSourceOnBattlefield));

        let mut out = vec![];
        e.apply_sbas(&mut out).unwrap();

        assert_eq!(
            e.state.objects.get(&src).map(|o| o.zone),
            Some(Zone::Graveyard),
            "anthem source dies on the first SBA pass"
        );
        assert_eq!(
            e.state.objects.get(&dependent).map(|o| o.zone),
            Some(Zone::Graveyard),
            "dependent must die on the SBA re-check once the anthem drains (CR 704.4)"
        );
    }

    #[test]
    fn surviving_deathtouch_history_expires_after_each_sba_check() {
        let mut e = engine();
        let target = add_creature(&mut e, 0, 3, 1);
        e.state.objects.get_mut(&target).unwrap().deathtouch_damage = true;
        e.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(target),
            kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Indestructible),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 0,
        });

        let mut out = vec![];
        e.apply_sbas(&mut out).unwrap();
        assert_eq!(
            e.state.objects.get(&target).map(|object| object.zone),
            Some(Zone::Battlefield),
            "indestructible prevents the deathtouch destruction"
        );
        assert!(!e.state.objects.get(&target).unwrap().deathtouch_damage);

        e.state.continuous_effects.clear();
        e.apply_sbas(&mut out).unwrap();
        assert_eq!(
            e.state.objects.get(&target).map(|object| object.zone),
            Some(Zone::Battlefield),
            "losing indestructible later must not revive stale deathtouch history"
        );
    }
}
