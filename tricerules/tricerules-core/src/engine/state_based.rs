use super::events::ev_log;
use super::resolution::{
    consume_regen_shield, destroy_permanent, permanent_moved_event, sacrifice_permanent,
};
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
        let mut changed = self.reindex_battlefield_control(out);
        changed |= self.refresh_enduring_story_designations();
        if self.state.pending_resolution.is_some() {
            return Ok(changed);
        }
        let zone_snapshot = self.snapshot_zone_event();
        let mut leaves: Vec<(TriggerSourceSnapshot, bool, bool)> = Vec::new();
        let mut tap_events = Vec::new();
        let candidate_ids: Vec<ObjectId> = self
            .state
            .objects
            .iter()
            .filter(|(_, object)| object.zone == Zone::Battlefield)
            .map(|(id, _)| *id)
            .collect();

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
            if let Ok(died) = destroy_permanent(&mut self.state, self.registry, id) {
                changed = true;
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
        }
        // Lethal-damage destroy: CR 614.8 / 701.19 regeneration shields apply before destruction.
        for id in to_destroy_lethal {
            let owner = self.state.objects.get(&id).map(|o| o.owner);
            let snapshot = death_snapshots.get(&id).cloned();
            let (regenerated, tap_event) = consume_regen_shield(self, id, out);
            if regenerated {
                changed = true;
                tap_events.extend(tap_event);
                let name = snapshot
                    .as_ref()
                    .map(|(source, _)| source.card_id.as_str())
                    .unwrap_or("creature");
                out.push(super::events::ev_log(format!("{name} regenerates.")));
            } else if let Ok(died) = destroy_permanent(&mut self.state, self.registry, id) {
                changed = true;
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
            if let Ok(died) = sacrifice_permanent(&mut self.state, self.registry, id) {
                changed = true;
                out.push(permanent_moved_event(
                    &self.state,
                    id,
                    owner,
                    rv1::permanent_moved::Destination::Graveyard,
                ));
                saga_leaves.push((source, was_creature, controller, died));
            }
        }

        if !leaves.is_empty() || !saga_leaves.is_empty() || !tap_events.is_empty() {
            let mut trigger_events = tap_events;
            trigger_events.extend(leaves.into_iter().flat_map(|(source, was_creature, died)| {
                leaves_and_dies_events(source, was_creature, died)
            }));
            trigger_events.extend(saga_leaves.into_iter().flat_map(
                |(source, was_creature, controller, died)| {
                    sacrifice_events(source, was_creature, controller, died)
                },
            ));
            self.fire_zone_triggers(zone_snapshot, trigger_events);
        }

        // CR 704.5n: Equipment attached to an illegal permanent becomes unattached but remains on
        // the battlefield. Use derived characteristics so future type-changing effects feed the
        // same SBA rather than teaching attachment state about those effects.
        let equipment_to_unattach: Vec<ObjectId> = self
            .state
            .objects
            .iter()
            .filter(|(_, eq)| {
                eq.zone == Zone::Battlefield
                    && self
                        .characteristics(eq.id)
                        .is_some_and(|value| value.has_type("Equipment"))
                    && eq.attached_to.is_some_and(|recipient| match recipient {
                        AttachmentRecipient::Player(_) => true,
                        AttachmentRecipient::Object(target_id) => {
                            !super::targeting::equipment_attachment_legal(self, eq.id, target_id)
                        }
                    })
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
            .filter(|(_, o)| o.zone != Zone::Battlefield && o.is_token())
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

        self.fire_zone_triggers(aura_zones, aura_events);
        if self.apply_legend_sbas(out)? {
            changed = true;
        }
        Ok(changed)
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
            return false;
        }

        for player in &mut self.state.players {
            player.battlefield.clear();
        }
        for (oid, controller) in desired {
            if let Some(index) = self.state.player_idx(controller) {
                self.state.players[index].battlefield.push(oid);
            }
        }

        if let Some(combat) = self.state.combat.as_mut() {
            let mut removed = Vec::new();
            for &oid in &changed_ids {
                let was_in_combat = combat.attacking.contains(&oid)
                    || combat.blockers.contains_key(&oid)
                    || combat
                        .blockers
                        .values()
                        .any(|blockers| blockers.contains(&oid));
                combat.attacking.retain(|&candidate| candidate != oid);
                combat.blockers.remove(&oid);
                for blockers in combat.blockers.values_mut() {
                    blockers.retain(|&candidate| candidate != oid);
                }
                if was_in_combat {
                    removed.push(oid);
                }
            }
            if !removed.is_empty() {
                out.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::RemovedFromCombat(
                        rv1::CreaturesRemovedFromCombat {
                            object_ids: removed,
                        },
                    )),
                });
            }
        }
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
            let Some(n) = characteristics.primary_name().map(str::to_string) else {
                continue;
            };
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
                        candidate_selectable: Vec::new(),
                        resolution_branches: Vec::new(),
                        mana_cost: String::new(),
                        generic_mana_cost: 0,
                        payment_currently_legal: false,
                        reveal_audience: 0,
                        revealed_zone_owner_player_id: None,
                        candidate_source_zones: Vec::new(),
                        combat_defender_options: Vec::new(),
                        waterbend: false,
                        selection_slots: Vec::new(),
                    },
                )),
            });
            out.push(ev_log(prompt.clone()));
            self.state.pending_resolution = Some(PendingResolution {
                deciding_player: controller,
                presentation: PendingResolutionPresentation {
                    source_object_id: first_id,
                    candidates: ids,
                    min: 1,
                    max: 1,
                    ordered: false,
                    prompt,
                    choice_kind: custom::ChoiceKind::LegendKeep,
                    unique_names: false,
                },
                continuation: ResolutionContinuation::LegendKeep,
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
                base_controller: owner,
                controller: owner,
                card_id: "walking_corpse".to_string(),
                token_origin: None,
                copiable_values: None,
                copy_revision: 0,
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
                tricerules_cards::primitives::GameCondition::SourceCounterCount {
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
