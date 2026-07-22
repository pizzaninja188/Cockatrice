use super::events::ev_log;
use super::resolution::{
    consume_regen_shield, destroy_permanent, permanent_moved_event, resolve_anthem_scope,
};
use super::*;

impl GameEngine {
    /// CR 613.4 / 604.3: true if continuous effect `e` applies to permanent `oid`.
    /// A `CreaturesMatching` scope (anthems/lords) is evaluated dynamically here so that
    /// creatures entering after the anthem was created are still affected.
    pub(super) fn effect_affects(&self, e: &ContinuousEffect, oid: ObjectId) -> bool {
        match &e.affected {
            AffectedScope::Single(id) => *id == oid,
            AffectedScope::AllCreatures => true,
            // Dynamic scope: true if the equipment `equip_oid` is currently attached to `oid`.
            AffectedScope::EquippedBy(equip_oid) => self
                .state
                .objects
                .get(equip_oid)
                .map(|eq| eq.zone == Zone::Battlefield && eq.attached_to == Some(oid))
                .unwrap_or(false),
            AffectedScope::CreaturesMatching {
                controller,
                subtype,
                color,
                exclude,
            } => {
                if *exclude == Some(oid) {
                    return false;
                }
                let Some(obj) = self.state.objects.get(&oid) else {
                    return false;
                };
                // CR 109.4: controller is the object's owner until control-changing exists.
                if let Some(pid) = controller {
                    if obj.owner != *pid {
                        return false;
                    }
                }
                let Some(def) = self.registry.get(&obj.card_id) else {
                    return false;
                };
                // CR 712.4: read creature/types from the active face for multi-face cards.
                let Some(face) = def.face(obj.face_up_index) else {
                    return false;
                };
                if !face.is_creature {
                    return false;
                }
                if let Some(sub) = subtype {
                    if !face.types.iter().any(|t| t == sub) {
                        return false;
                    }
                }
                if let Some(c) = color {
                    if !def.colors().contains(c) {
                        return false;
                    }
                }
                true
            }
            // Player-scoped effects (e.g. ExtraLandPlays) don't affect permanents.
            AffectedScope::Player(_) => false,
        }
    }

    /// Sum of extra land plays granted to `pid` by active `ExtraLandPlays` continuous effects.
    pub(super) fn extra_land_plays_for(&self, pid: PlayerId) -> u32 {
        self.state
            .continuous_effects
            .iter()
            .filter_map(|e| {
                if let AffectedScope::Player(p) = e.affected {
                    if p == pid {
                        if let ContinuousEffectKind::ExtraLandPlays(n) = e.kind {
                            return Some(n);
                        }
                    }
                }
                None
            })
            .sum()
    }

    /// CR 604.3 / 611.3: when a permanent with static anthem abilities enters the battlefield,
    /// push the corresponding `WhileSourceOnBattlefield` continuous effects. The LTB drain in
    /// `move_object_to_zone` removes them when the source leaves.
    pub(super) fn emit_static_abilities_on_enter(&mut self, object_id: ObjectId) {
        let Some(obj) = self.state.objects.get(&object_id) else {
            return;
        };
        let controller = obj.owner;
        let card_id = obj.card_id.clone();
        let face_up_index = obj.face_up_index;
        // CR 712.4: static abilities are read from the active face (face_up_index).
        let statics: Vec<StaticAbilityDef> = match self.registry.get(&card_id) {
            Some(def) => def
                .face(face_up_index)
                .map(|f| f.static_abilities.to_vec())
                .unwrap_or_default(),
            None => return,
        };
        let timestamp = self.state.command_index;
        for sa in statics {
            match sa {
                StaticAbilityDef::AnthemPt {
                    filter,
                    delta_power,
                    delta_toughness,
                } => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        source_id: Some(object_id),
                        affected: resolve_anthem_scope(&filter, controller, object_id),
                        kind: ContinuousEffectKind::PtModify {
                            delta_power,
                            delta_toughness,
                        },
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
                // CR 303.4c / CR 613.1b: an aura's P/T modification applies continuously while it
                // is attached to the enchanted permanent. Scoped to `Single(enchanted_oid)` and
                // drained at LTB (WhileSourceOnBattlefield) by the existing move_object_to_zone
                // drain. `attached_to` is set before fire_triggers so this reads the correct target.
                StaticAbilityDef::AuraPtModify {
                    delta_power,
                    delta_toughness,
                } => {
                    let Some(enchanted_oid) = self
                        .state
                        .objects
                        .get(&object_id)
                        .and_then(|o| o.attached_to)
                    else {
                        continue;
                    };
                    self.state.continuous_effects.push(ContinuousEffect {
                        source_id: Some(object_id),
                        affected: AffectedScope::Single(enchanted_oid),
                        kind: ContinuousEffectKind::PtModify {
                            delta_power,
                            delta_toughness,
                        },
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
                // CR 301.5b / 702.6: equipment P/T bonus — scope is EquippedBy(equipment_oid)
                // so the boost follows re-equip without recreating the continuous effect.
                StaticAbilityDef::EquippedBonus {
                    delta_power,
                    delta_toughness,
                } => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        source_id: Some(object_id),
                        affected: AffectedScope::EquippedBy(object_id),
                        kind: ContinuousEffectKind::PtModify {
                            delta_power,
                            delta_toughness,
                        },
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
                StaticAbilityDef::AnthemKeyword { filter, keyword } => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        source_id: Some(object_id),
                        affected: resolve_anthem_scope(&filter, controller, object_id),
                        kind: ContinuousEffectKind::Layer6AddKeyword(keyword),
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
                // CR 305.2b / layer 5: player-scoped extra land plays while the source is on the
                // battlefield (Exploration, Oracle of Mul Daya). Drains at LTB via the same
                // WhileSourceOnBattlefield path.
                StaticAbilityDef::ExtraLandPlays { count } => {
                    self.state.continuous_effects.push(ContinuousEffect {
                        source_id: Some(object_id),
                        affected: AffectedScope::Player(controller),
                        kind: ContinuousEffectKind::ExtraLandPlays(count),
                        duration: EffectDuration::WhileSourceOnBattlefield,
                        timestamp,
                    });
                }
            }
        }
    }

    /// Effective (rules-visible) power of `oid`: base from the active face (CR 712.4), then
    /// CR 613.4 layer 7c modifying continuous effects, then layer 7d +1/+1 / -1/-1 counters.
    /// Returns `None` for non-creatures (no base power on the active face).
    pub fn effective_power(&self, oid: ObjectId) -> Option<u32> {
        let obj = self.state.objects.get(&oid)?;
        // CR 712.4: for multi-face permanents (obj.power == None), read base P/T from the active
        // face in the registry.  For normal cards obj.power caches the definition value and is
        // always Some; prefer it so test overrides and token snapshots are respected.
        let base = if let Some(p) = obj.power {
            p as i32
        } else {
            self.registry
                .get(&obj.card_id)
                .and_then(|d| d.face(obj.face_up_index))
                .and_then(|f| f.power)? as i32
        };
        let delta: i32 = self
            .state
            .continuous_effects
            .iter()
            .filter(|e| self.effect_affects(e, oid))
            .map(|e| match &e.kind {
                ContinuousEffectKind::PtModify { delta_power, .. } => *delta_power,
                ContinuousEffectKind::Layer6AddKeyword(_) => 0,
                ContinuousEffectKind::ExtraLandPlays(_) => 0,
            })
            .sum();
        // Layer 7d: counters apply after all layer-7c modifying effects (CR 613.4d).
        Some((base + delta + obj.counter_pt_delta()).max(0) as u32)
    }

    /// Effective (rules-visible) toughness of `oid`: base from the active face (CR 712.4),
    /// then layer-7c continuous effects, then layer-7d counters (CR 613.4).
    pub fn effective_toughness(&self, oid: ObjectId) -> Option<u32> {
        let obj = self.state.objects.get(&oid)?;
        // CR 712.4: for multi-face permanents (obj.toughness == None), read base toughness from
        // the active face in the registry.  For normal cards obj.toughness caches the definition
        // value and is always Some; prefer it so test overrides and token snapshots are respected.
        let base = if let Some(t) = obj.toughness {
            t as i32
        } else {
            self.registry
                .get(&obj.card_id)
                .and_then(|d| d.face(obj.face_up_index))
                .and_then(|f| f.toughness)? as i32
        };
        let delta: i32 = self
            .state
            .continuous_effects
            .iter()
            .filter(|e| self.effect_affects(e, oid))
            .map(|e| match &e.kind {
                ContinuousEffectKind::PtModify {
                    delta_toughness, ..
                } => *delta_toughness,
                ContinuousEffectKind::Layer6AddKeyword(_) => 0,
                ContinuousEffectKind::ExtraLandPlays(_) => 0,
            })
            .sum();
        Some((base + delta + obj.counter_pt_delta()).max(0) as u32)
    }

    /// Effective keyword check: true if `oid` has `kw` either from its card definition (static)
    /// or from an active `Layer6AddKeyword` continuous effect (CR 613 layer 6). Use this instead
    /// of `GameObject::has_keyword` wherever granted keywords must be respected (combat legality,
    /// SBA checks, damage resolution).
    pub fn effective_has_keyword(&self, oid: ObjectId, kw: Keyword) -> bool {
        if let Some(obj) = self.state.objects.get(&oid) {
            if obj.has_keyword(self.registry, kw) {
                return true;
            }
        }
        self.state.continuous_effects.iter().any(|e| {
            matches!(&e.kind, ContinuousEffectKind::Layer6AddKeyword(k) if *k == kw)
                && self.effect_affects(e, oid)
        })
    }

    /// CR 514.2: drain all UntilEndOfTurn continuous effects. Called from
    /// `finish_cleanup_roll_new_turn`, after CR 514.1 discards have completed.
    pub(super) fn cleanup_until_end_of_turn_creature_pt(&mut self) {
        self.state
            .continuous_effects
            .retain(|e| e.duration != EffectDuration::UntilEndOfTurn);
    }

    /// CR 514.2: damage marked on permanents is removed during cleanup. Regeneration shields
    /// also expire at end of turn (CR 701.15a "this turn").
    pub(super) fn cleanup_marked_damage(&mut self) {
        for o in self.state.objects.values_mut() {
            if o.zone == Zone::Battlefield {
                o.damage = 0;
                o.deathtouch_damage = false;
                o.regeneration_shields = 0;
            }
        }
        // CR 614.1a: damage prevention shields (Healing Salve, Circle of Protection, etc.)
        // and the global Fog flag expire at the cleanup step alongside marked damage.
        self.state.damage_prevention_shields.clear();
        self.state.prevent_all_combat_damage_this_turn = false;
    }

    /// CR 704.4: state-based actions are checked and performed repeatedly until a check finds
    /// nothing left to do. Stops early if a legend-rule SBA pauses for player choice.
    pub(super) fn apply_sbas(&mut self, out: &mut Vec<rv1::RuledEvent>) -> Result<(), EngineError> {
        while self.state.pending_resolution.is_none() && self.apply_sbas_once(out)? {}
        Ok(())
    }

    /// One state-based-action pass (CR 704.5). Returns `true` if it changed game state.
    pub(super) fn apply_sbas_once(
        &mut self,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let mut changed = false;
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
            .filter(|(_, o)| o.zone == Zone::Battlefield && o.toughness.is_some())
            .map(|(id, _)| *id)
            .collect();

        // CR 704.5f: toughness-0 deaths — not regeneratable (this is a different SBA from destroy).
        let mut to_destroy_t0 = Vec::new();
        // CR 704.5g/704.5h: lethal-damage deaths — regeneration shields apply here.
        let mut to_destroy_lethal = Vec::new();
        for id in candidate_ids {
            let Some(eff_t) = self.effective_toughness(id) else {
                continue;
            };
            let indestructible = self.effective_has_keyword(id, Keyword::Indestructible);
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
            let card_id_for_trigger = self.state.objects.get(&id).map(|o| o.card_id.clone());
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
                if let (Some(cid), Some(ctrl)) = (card_id_for_trigger, owner) {
                    self.fire_triggers(
                        GameEvent::Dies {
                            object_id: id,
                            card_id: cid,
                            controller: ctrl,
                        },
                        out,
                    );
                }
            }
        }
        // Lethal-damage destroy: CR 701.15 regeneration shields apply before destruction.
        for id in to_destroy_lethal {
            let owner = self.state.objects.get(&id).map(|o| o.owner);
            let card_id_for_trigger = self.state.objects.get(&id).map(|o| o.card_id.clone());
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
                if let (Some(cid), Some(ctrl)) = (card_id_for_trigger, owner) {
                    self.fire_triggers(
                        GameEvent::Dies {
                            object_id: id,
                            card_id: cid,
                            controller: ctrl,
                        },
                        out,
                    );
                }
            }
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
            if let Some(o) = self.state.objects.remove(&id) {
                changed = true;
                if let Some(pidx) = self.state.player_idx(o.owner) {
                    let p = &mut self.state.players[pidx];
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
                        .registry
                        .get(&o.card_id)
                        .map(|d| d.is_aura)
                        .unwrap_or(false)
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
            let card_id_for_trigger = self.state.objects.get(&id).map(|o| o.card_id.clone());
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
                if let (Some(cid), Some(ctrl)) = (card_id_for_trigger, owner) {
                    self.fire_triggers(
                        GameEvent::Dies {
                            object_id: id,
                            card_id: cid,
                            controller: ctrl,
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
    /// one legend conflict at a time via `ResolutionChoiceRequired` (choice_kind 5); the SBA
    /// loop stops while waiting for the choice and resumes after `SubmitResolutionChoice`.
    pub(super) fn apply_legend_sbas(
        &mut self,
        out: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let mut by_owner_name: BTreeMap<(PlayerId, String), Vec<ObjectId>> = BTreeMap::new();
        for (&id, o) in &self.state.objects {
            if o.zone != Zone::Battlefield {
                continue;
            }
            if !self
                .registry
                .get(&o.card_id)
                .map(|c| c.is_legendary)
                .unwrap_or(false)
            {
                continue;
            }
            let n = self.registry.get(&o.card_id).unwrap().name.clone();
            by_owner_name.entry((o.owner, n)).or_default().push(id);
        }
        // Process one legend conflict per SBA pass; after the player's choice the SBA loop
        // re-runs and finds any remaining conflicts.
        for ((owner, name), mut ids) in by_owner_name {
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
                        deciding_player_id: owner,
                        source_object_id: first_id,
                        prompt_text: prompt.clone(),
                        // choice_kind 5 = legend rule: pick one battlefield permanent to keep.
                        // Public (battlefield); the client selects it by clicking the permanent.
                        choice_kind: 5,
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
                controller: owner,
                card_id: String::new(),
                targets: vec![],
                ability_text: None,
                source_permanent_id: None,
                ability_index: None,
                is_triggered: false,
                is_copy: false,
                face_index: 0,
                chosen_x: 0,
                target_damage: vec![],
            };
            self.state.pending_resolution = Some(PendingResolution {
                item: dummy_item,
                custom_key: "__legend_sba".to_string(),
                step: 0,
                scratch: vec![],
                deciding_player: owner,
                candidates: ids,
                min: 1,
                max: 1,
                ordered: false,
                prompt,
                choice_kind: 5,
                unique_names: false,
                copy_source_object_id: 0,
                search_destination: SearchDestination::default(),
                search_shuffle: false,
                search_reveal: false,
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
        move_object_to_zone(&mut e.state, src, Zone::Graveyard).unwrap();
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
        move_object_to_zone(&mut e.state, src, Zone::Graveyard).unwrap();
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
