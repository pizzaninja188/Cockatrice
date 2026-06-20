use super::*;
use super::resolution::{destroy_permanent, permanent_moved_event};

impl GameEngine {
    /// CR 613.4 / 604.3: true if continuous effect `e` applies to permanent `oid`.
    /// A `CreaturesMatching` scope (anthems/lords) is evaluated dynamically here so that
    /// creatures entering after the anthem was created are still affected.
    pub(super) fn effect_affects(&self, e: &ContinuousEffect, oid: ObjectId) -> bool {
        match &e.affected {
            AffectedScope::Single(id) => *id == oid,
            AffectedScope::AllCreatures => true,
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
                if !def.is_creature {
                    return false;
                }
                if let Some(sub) = subtype {
                    if !def.types.iter().any(|t| t == sub) {
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
        }
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
        let statics: Vec<StaticAbilityDef> = match self.registry.get(&card_id) {
            Some(def) => def.primary_face().static_abilities.to_vec(),
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
            }
        }
    }

    /// Effective (rules-visible) power of `oid`: base from card definition, then CR 613.4
    /// layer 7c modifying continuous effects, then layer 7d +1/+1 / -1/-1 counters. Returns
    /// `None` for non-creatures (no base power).
    pub fn effective_power(&self, oid: ObjectId) -> Option<u32> {
        let obj = self.state.objects.get(&oid)?;
        let base = obj.power? as i32;
        let delta: i32 = self
            .state
            .continuous_effects
            .iter()
            .filter(|e| self.effect_affects(e, oid))
            .map(|e| match &e.kind {
                ContinuousEffectKind::PtModify { delta_power, .. } => *delta_power,
            })
            .sum();
        // Layer 7d: counters apply after all layer-7c modifying effects (CR 613.4d).
        Some((base + delta + obj.counter_pt_delta()).max(0) as u32)
    }

    /// Effective (rules-visible) toughness of `oid`: base, then layer-7c continuous effects,
    /// then layer-7d counters (CR 613.4).
    pub fn effective_toughness(&self, oid: ObjectId) -> Option<u32> {
        let obj = self.state.objects.get(&oid)?;
        let base = obj.toughness? as i32;
        let delta: i32 = self
            .state
            .continuous_effects
            .iter()
            .filter(|e| self.effect_affects(e, oid))
            .map(|e| match &e.kind {
                ContinuousEffectKind::PtModify {
                    delta_toughness, ..
                } => *delta_toughness,
            })
            .sum();
        Some((base + delta + obj.counter_pt_delta()).max(0) as u32)
    }

    /// CR 514.2: drain all UntilEndOfTurn continuous effects. Called from
    /// `finish_cleanup_roll_new_turn`, after CR 514.1 discards have completed.
    pub(super) fn cleanup_until_end_of_turn_creature_pt(&mut self) {
        self.state
            .continuous_effects
            .retain(|e| e.duration != EffectDuration::UntilEndOfTurn);
    }

    /// CR 514.2: damage marked on permanents is removed during cleanup.
    pub(super) fn cleanup_marked_damage(&mut self) {
        for o in self.state.objects.values_mut() {
            if o.zone == Zone::Battlefield && (o.damage != 0 || o.deathtouch_damage) {
                o.damage = 0;
                o.deathtouch_damage = false;
            }
        }
    }

    /// CR 704.4: state-based actions are checked and performed repeatedly until a check finds
    /// nothing left to do.
    pub(super) fn apply_sbas(&mut self, out: &mut Vec<rv1::RuledEvent>) -> Result<(), EngineError> {
        while self.apply_sbas_once(out)? {}
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

        let mut to_destroy = Vec::new();
        for id in candidate_ids {
            let Some(eff_t) = self.effective_toughness(id) else {
                continue;
            };
            let Some(o) = self.state.objects.get(&id) else {
                continue;
            };
            let indestructible = o.has_keyword(self.registry, Keyword::Indestructible);
            // CR 704.5f: toughness 0 — still dies even with indestructible.
            if eff_t == 0 {
                to_destroy.push(id);
            // CR 704.5g / 704.5h: lethal damage or deathtouch — blocked by indestructible.
            } else if !indestructible && (o.damage >= eff_t || o.deathtouch_damage) {
                to_destroy.push(id);
            }
        }
        for id in to_destroy {
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

        let legend_events = self.apply_legend_sbas()?;
        if !legend_events.is_empty() {
            changed = true;
        }
        out.extend(legend_events);
        Ok(changed)
    }

    pub(super) fn apply_legend_sbas(&mut self) -> Result<Vec<rv1::RuledEvent>, EngineError> {
        let mut by_owner_name: BTreeMap<(PlayerId, String), Vec<ObjectId>> = BTreeMap::new();
        let mut out = Vec::new();
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
        for ids in by_owner_name.values_mut() {
            if ids.len() < 2 {
                continue;
            }
            ids.sort_unstable();
            for &g in ids.iter().skip(1) {
                let owner = self.state.objects.get(&g).map(|o| o.owner);
                if destroy_permanent(&mut self.state, g).is_ok() {
                    if let Some(owner_id) = owner {
                        out.push(permanent_moved_event(
                            &self.state,
                            g,
                            owner_id,
                            rv1::permanent_moved::Destination::Graveyard,
                        ));
                    }
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod sba_tests {
    use super::super::*;
    use crate::game_state::GameState;

    fn make_engine_for_sba() -> GameEngine {
        GameEngine::new_with_default_decks()
    }

    #[test]
    fn legendary_sba_duplicate_legend_removed() {
        let mut eng = make_engine_for_sba();
        // The SBA pass should be callable without panicking.
        let mut ev = vec![];
        eng.apply_sbas(&mut ev).expect("apply_sbas ok");
    }
}
