use super::events::ev_log;
use super::resolution::{consume_regen_shield, destroy_permanent, permanent_moved_event};
use super::*;

impl GameEngine {
    /// CR 704.4: state-based actions are checked and performed repeatedly until a check finds
    /// nothing left to do. Stops early if a legend-rule SBA pauses for player choice.
    pub(super) fn apply_sbas(&mut self, out: &mut Vec<rv1::RuledEvent>) -> Result<(), EngineError> {
        while self.state.pending_resolution.is_none() && self.apply_sbas_once(out)? {}
        self.debug_assert_battlefield_control_index();
        Ok(())
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
    ) -> Result<bool, EngineError> {
        let mut changed = false;
        let mut dies = Vec::new();
        // CR 122.3: counter annihilation (+1/+1 and -1/-1 pairs cancel).
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
            }
        }

        let candidate_ids: Vec<ObjectId> = self
            .state
            .objects
            .iter()
            .filter(|(id, object)| {
                object.zone == Zone::Battlefield
                    && self
                        .characteristics(**id)
                        .is_some_and(|value| value.toughness.is_some())
            })
            .map(|(id, _)| *id)
            .collect();

        // CR 704.5f: toughness-0 deaths — not regeneratable (this is a different SBA from destroy).
        let mut to_destroy_t0 = Vec::new();
        // CR 704.5g/704.5h: lethal-damage deaths — regeneration shields apply here.
        let mut to_destroy_lethal = Vec::new();
        for id in candidate_ids {
            let Some(characteristics) = self.characteristics(id) else {
                continue;
            };
            let Some(eff_t) = characteristics.toughness else {
                continue;
            };
            let indestructible = characteristics.has_keyword(Keyword::Indestructible);
            let Some(o) = self.state.objects.get(&id) else {
                continue;
            };
            // CR 704.5f: toughness 0 — still dies even with indestructible.
            if eff_t == 0 {
                to_destroy_t0.push(id);
            } else if !indestructible && (o.damage >= eff_t || o.deathtouch_damage) {
                to_destroy_lethal.push(id);
            }
        }
        // Toughness-0: bypass regeneration (CR 704.5f — not a "destroy" trigger).
        for id in to_destroy_t0 {
            let owner = self.state.objects.get(&id).map(|o| o.owner);
            let controller = self.state.objects.get(&id).map(|o| o.controller);
            let card_id_for_trigger = self.state.objects.get(&id).map(|o| o.card_id.clone());
            let was_creature = self
                .characteristics(id)
                .is_some_and(|value| value.is_creature());
            if destroy_permanent(&mut self.state, id).is_ok() {
                changed = true;
                if let Some(owner_id) = owner {
                    out.push(permanent_moved_event(
                        &self.state,
                        id,
                        owner_id,
                        rv1::permanent_moved::Destination::Graveyard,
                    ));
                }
                if let (Some(cid), Some(ctrl)) = (card_id_for_trigger, controller) {
                    dies.push((id, cid, ctrl, was_creature));
                }
            }
        }
        // Lethal-damage destroy: CR 701.15 regeneration shields apply before destruction.
        for id in to_destroy_lethal {
            let owner = self.state.objects.get(&id).map(|o| o.owner);
            let controller = self.state.objects.get(&id).map(|o| o.controller);
            let card_id_for_trigger = self.state.objects.get(&id).map(|o| o.card_id.clone());
            let was_creature = self
                .characteristics(id)
                .is_some_and(|value| value.is_creature());
            if consume_regen_shield(&mut self.state, id, out) {
                changed = true;
                let name = card_id_for_trigger.as_deref().unwrap_or("creature");
                out.push(super::events::ev_log(format!("{name} regenerates.")));
            } else if destroy_permanent(&mut self.state, id).is_ok() {
                changed = true;
                if let Some(owner_id) = owner {
                    out.push(permanent_moved_event(
                        &self.state,
                        id,
                        owner_id,
                        rv1::permanent_moved::Destination::Graveyard,
                    ));
                }
                if let (Some(cid), Some(ctrl)) = (card_id_for_trigger, controller) {
                    dies.push((id, cid, ctrl, was_creature));
                }
            }
        }

        if !dies.is_empty() {
            self.fire_dies_batch(&dies, out);
        }

        // CR 704.5p: equipment falls off if the attached creature is no longer on the battlefield.
        let equipment_to_unattach: Vec<ObjectId> = self
            .state
            .objects
            .iter()
            .filter(|(_, eq)| {
                eq.zone == Zone::Battlefield
                    && eq
                        .attached_to
                        .map(|target_id| {
                            self.state
                                .objects
                                .get(&target_id)
                                .map(|t| t.zone != Zone::Battlefield)
                                .unwrap_or(true)
                        })
                        .unwrap_or(false)
            })
            .map(|(id, _)| *id)
            .collect();
        for eq_id in equipment_to_unattach {
            if let Some(eq) = self.state.objects.get_mut(&eq_id) {
                eq.attached_to = None;
                changed = true;
            }
        }

        // CR 111.7/111.8: tokens that have left the battlefield cease to exist.
        let vanished: Vec<ObjectId> = self
            .state
            .objects
            .iter()
            .filter(|(_, o)| o.zone != Zone::Battlefield && o.is_token(self.registry))
            .map(|(id, _)| *id)
            .collect();
        for id in vanished {
            if self.state.objects.remove(&id).is_some() {
                changed = true;
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
                    && o.attached_to
                        .map(|eid| {
                            self.state
                                .objects
                                .get(&eid)
                                .map(|e| e.zone != Zone::Battlefield)
                                .unwrap_or(true)
                        })
                        .unwrap_or(true)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in orphaned_auras {
            let owner = self.state.objects.get(&id).map(|o| o.owner);
            let controller = self.state.objects.get(&id).map(|o| o.controller);
            let card_id_for_trigger = self.state.objects.get(&id).map(|o| o.card_id.clone());
            let was_creature = self
                .characteristics(id)
                .is_some_and(|value| value.is_creature());
            if destroy_permanent(&mut self.state, id).is_ok() {
                changed = true;
                if let Some(owner_id) = owner {
                    out.push(permanent_moved_event(
                        &self.state,
                        id,
                        owner_id,
                        rv1::permanent_moved::Destination::Graveyard,
                    ));
                }
                if let (Some(cid), Some(ctrl)) = (card_id_for_trigger, controller) {
                    self.fire_triggers(
                        GameEvent::Dies {
                            object_id: id,
                            card_id: cid,
                            controller: ctrl,
                            was_creature,
                        },
                        out,
                    );
                }
            }
        }

        if self.apply_legend_sbas(out)? {
            changed = true;
        }
        Ok(changed)
    }

    /// CR 704.5j: if a player controls two or more legendary permanents with the same name,
    /// that player chooses one to keep; the rest go to their owners' graveyards. Processes
    /// one legend conflict at a time via `ResolutionChoiceRequired` (`ChoiceKind::LegendKeep`); the SBA
    /// loop stops while waiting for the choice and resumes after `SubmitResolutionChoice`.
    pub(super) fn apply_legend_sbas(
        &mut self,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let mut by_controller_name: BTreeMap<(PlayerId, String), Vec<ObjectId>> = BTreeMap::new();
        for (&id, o) in &self.state.objects {
            if o.zone != Zone::Battlefield {
                continue;
            }
            let Some(characteristics) = self.characteristics(id) else {
                continue;
            };
            if !characteristics.is_legendary() {
                continue;
            }
            let n = self.registry.get(&o.card_id).unwrap().name.clone();
            by_controller_name
                .entry((characteristics.controller, n))
                .or_default()
                .push(id);
        }
        // Process one legend conflict per SBA pass; after the player's choice the SBA loop
        // re-runs and finds any remaining conflicts.
        for ((controller, name), mut ids) in by_controller_name {
            if ids.len() < 2 {
                continue;
            }
            ids.sort_unstable();
            let first_id = ids[0];
            let candidate_card_ids: Vec<String> = ids
                .iter()
                .map(|&oid| {
                    self.state
                        .objects
                        .get(&oid)
                        .map(|o| o.card_id.clone())
                        .unwrap_or_default()
                })
                .collect();
            let candidate_names: Vec<String> = ids.iter().map(|_| name.clone()).collect();
            let prompt = format!("Legend rule: choose which {name} to keep (CR 704.5j)");
            out.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                    rv1::ResolutionChoiceRequired {
                        deciding_player_id: controller,
                        source_object_id: first_id,
                        prompt_text: prompt.clone(),
                        // LegendKeep: pick one battlefield permanent to keep. Public
                        // (battlefield); the client selects it by clicking the permanent.
                        choice_kind: custom::ChoiceKind::LegendKeep as i32,
                        candidate_object_ids: ids.clone(),
                        candidate_card_ids,
                        candidate_names,
                        min: 1,
                        max: 1,
                        ordered: false,
                        unique_names: false,
                        candidate_server_card_ids: vec![],
                    },
                )),
            });
            out.push(ev_log(prompt.clone()));
            // Dummy stack item — legend SBA has no spell; `id` is the first candidate so
            // `source_object_id` points to one of the duplicate legends.
            let dummy_item = StackItem {
                id: first_id,
                controller,
                card_id: String::new(),
                targets: vec![],
                ability_text: None,
                source_permanent_id: None,
                source_zone_change: 0,
                ability_index: None,
                is_triggered: false,
                is_copy: false,
                face_index: 0,
                chosen_x: 0,
                target_damage: vec![],
                chosen_modes: vec![],
                trigger_player: None,
                flashback: false,
            };
            self.state.pending_resolution = Some(PendingResolution {
                item: dummy_item,
                custom_key: "__legend_sba".to_string(),
                step: 0,
                scratch: vec![],
                deciding_player: controller,
                candidates: ids,
                min: 1,
                max: 1,
                ordered: false,
                prompt,
                choice_kind: custom::ChoiceKind::LegendKeep,
                unique_names: false,
                copy_source_object_id: 0,
                search_destination: SearchDestination::default(),
                search_shuffle: false,
                search_reveal: false,
                // An SBA, not a spell resolution: `item` is a dummy with no effect list.
                resume_effect_index: None,
            });
            return Ok(true);
        }
        Ok(false)
    }
}

#[cfg(test)]
mod sba_tests {
    use super::super::resolution::move_object_to_zone;
    use super::*;

    fn engine() -> GameEngine {
        GameEngine::new_with_default_decks(1, &[0, 1], 20).expect("new")
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
                controller: owner,
                card_id: "walking_corpse".to_string(),
                zone: Zone::Battlefield,
                tapped: false,
                summoning_sick: false,
                power: Some(2),
                toughness: Some(toughness),
                damage,
                deathtouch_damage: false,
                counters: Default::default(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
            },
        );
        let idx = e.state.player_idx(owner).unwrap();
        e.state.players[idx].battlefield.push(id);
        id
    }

    fn anthem(source: ObjectId, dt: i32, duration: EffectDuration) -> ContinuousEffect {
        ContinuousEffect {
            source_id: Some(source),
            affected: AffectedScope::AllCreatures,
            kind: ContinuousEffectKind::PtModify {
                delta_power: 0,
                delta_toughness: dt,
            },
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
        move_object_to_zone(&mut e.state, src, Zone::Graveyard, None).unwrap();
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
            source_id: Some(src),
            affected: AffectedScope::Single(target),
            kind: ContinuousEffectKind::PtModify {
                delta_power: 0,
                delta_toughness: 2,
            },
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 0,
        });
        move_object_to_zone(&mut e.state, src, Zone::Graveyard, None).unwrap();
        assert_eq!(e.effective_toughness(target), Some(3)); // still buffed
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
}
