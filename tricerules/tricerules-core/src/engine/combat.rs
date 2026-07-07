use super::events::{ev_log, ev_phase_labeled, ev_priority_changed, object_display_name};
use super::legal_actions::fill_legal;
use super::*;

impl GameEngine {
    /// Returns false if `blocker_id` is not permitted to block `attacker_id` due to
    /// keyword evasion abilities. Checks all active blocking restrictions in order.
    pub(super) fn can_block(&self, attacker_id: ObjectId, blocker_id: ObjectId) -> bool {
        use tricerules_cards::Keyword;
        let Some(att) = self.state.objects.get(&attacker_id) else {
            return false;
        };
        let Some(blk) = self.state.objects.get(&blocker_id) else {
            return false;
        };

        // CR 702.9b — flying: can only be blocked by creatures with flying or reach.
        if self.effective_has_keyword(attacker_id, Keyword::Flying)
            && !self.effective_has_keyword(blocker_id, Keyword::Flying)
            && !self.effective_has_keyword(blocker_id, Keyword::Reach)
        {
            return false;
        }

        // CR 702.13b — intimidate: can only be blocked by artifact creatures and/or
        // creatures that share a color with the intimidate creature.
        if self.effective_has_keyword(attacker_id, Keyword::Intimidate) {
            let blk_def = self.registry.get(&blk.card_id);
            let blk_is_artifact = blk_def.map(|d| d.is_artifact).unwrap_or(false);
            if !blk_is_artifact {
                let att_colors = self
                    .registry
                    .get(&att.card_id)
                    .map(|d| d.colors())
                    .unwrap_or_default();
                let blk_colors = blk_def.map(|d| d.colors()).unwrap_or_default();
                let shares_color = att_colors.iter().any(|c| blk_colors.contains(c));
                if !shares_color {
                    return false;
                }
            }
        }

        true
    }

    pub(super) fn active_player_has_eligible_attackers(&self) -> bool {
        let ap = self.state.active_player_id();
        let Some(ap_idx) = self.state.player_idx(ap) else {
            return false;
        };
        self.state.players[ap_idx].battlefield.iter().any(|oid| {
            self.state.objects.get(oid).is_some_and(|o| {
                // CR 702.10b: Haste lets a creature attack (and use {T} abilities) even if it
                // just entered the battlefield this turn (i.e. ignore summoning sickness).
                let effectively_sick = o.summoning_sick
                    && !self.effective_has_keyword(*oid, tricerules_cards::Keyword::Haste);
                // CR 702.3b: defenders can't attack, so they don't count as eligible attackers.
                let has_defender =
                    self.effective_has_keyword(*oid, tricerules_cards::Keyword::Defender);
                o.zone == Zone::Battlefield
                    && o.owner == ap
                    && o.is_creature(self.registry)
                    && !effectively_sick
                    && !o.tapped
                    && !has_defender
            })
        })
    }

    pub(super) fn defending_player_has_eligible_blockers(&self) -> bool {
        use tricerules_cards::Keyword;
        let Some(dp) = self.state.defending_player_id_1v1() else {
            return false;
        };
        let Some(dp_idx) = self.state.player_idx(dp) else {
            return false;
        };
        let attacking: Vec<ObjectId> = self
            .state
            .combat
            .as_ref()
            .map(|c| c.attacking.clone())
            .unwrap_or_default();
        if attacking.is_empty() {
            return false;
        }
        // CR 302.6: summoning sickness does NOT prevent blocking.
        // Build the full list of untapped defender creatures up-front so the menace
        // check can count potential co-blockers without re-scanning the battlefield.
        let defenders: Vec<ObjectId> = self.state.players[dp_idx]
            .battlefield
            .iter()
            .filter(|&&oid| {
                self.state.objects.get(&oid).is_some_and(|o| {
                    o.zone == Zone::Battlefield
                        && o.owner == dp
                        && o.is_creature(self.registry)
                        && !o.tapped
                })
            })
            .copied()
            .collect();
        // A legal non-empty blocking assignment exists only when at least one defender
        // creature can participate in a valid block. For menace attackers (CR 702.111),
        // participation requires at least one OTHER defender that can block the same
        // attacker — otherwise the only achievable result is an illegal single-blocker.
        defenders.iter().any(|&cid| {
            attacking.iter().any(|&aid| {
                if !self.can_block(aid, cid) {
                    return false;
                }
                let has_menace = self.effective_has_keyword(aid, Keyword::Menace);
                if has_menace {
                    // Need at least one other defender that can also block this attacker.
                    defenders
                        .iter()
                        .any(|&other| other != cid && self.can_block(aid, other))
                } else {
                    true
                }
            })
        })
    }

    /// CR 508.1d: the active player's creatures that MUST be declared as attackers this combat —
    /// untapped, not summoning-sick (unless Haste), non-Defender creatures with `must_attack_if_able`,
    /// when a defending player exists to attack. Single source of truth shared by `set_attackers`
    /// enforcement and the client-facing `LegalActions` gate (Juggernaut, Goblin Brigand, Crazed Goblin).
    pub(super) fn required_attacker_ids(&self) -> Vec<ObjectId> {
        use tricerules_cards::Keyword;
        if self.state.defending_player_id_1v1().is_none() {
            return Vec::new();
        }
        let ap = self.state.active_player_id();
        let Some(ap_idx) = self.state.player_idx(ap) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for &oid in &self.state.players[ap_idx].battlefield {
            let Some(obj) = self.state.objects.get(&oid) else {
                continue;
            };
            if !obj.must_attack_if_able {
                continue;
            }
            if !obj.is_creature(self.registry) {
                continue;
            }
            if obj.tapped {
                continue;
            }
            if obj.summoning_sick && !obj.has_keyword(self.registry, Keyword::Haste) {
                continue;
            }
            if obj.has_keyword(self.registry, Keyword::Defender) {
                continue;
            }
            out.push(oid);
        }
        out
    }

    /// CR 509.1c: the defending player's creatures that MUST be declared as blockers this combat —
    /// untapped creatures with `must_block_if_able` that can legally block at least one declared
    /// attacker. Single source of truth shared by `set_blockers` enforcement and the client-facing
    /// `LegalActions` gate. Empty until attackers are declared.
    pub(super) fn required_blocker_ids(&self) -> Vec<ObjectId> {
        let Some(defending_player) = self.state.defending_player_id_1v1() else {
            return Vec::new();
        };
        let attacking: Vec<ObjectId> = self
            .state
            .combat
            .as_ref()
            .map(|c| c.attacking.clone())
            .unwrap_or_default();
        if attacking.is_empty() {
            return Vec::new();
        }
        let Some(dp_idx) = self.state.player_idx(defending_player) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for &oid in &self.state.players[dp_idx].battlefield {
            let Some(obj) = self.state.objects.get(&oid) else {
                continue;
            };
            if !obj.must_block_if_able {
                continue;
            }
            if !obj.is_creature(self.registry) {
                continue;
            }
            if obj.tapped {
                continue;
            }
            if attacking.iter().any(|&aid| self.can_block(aid, oid)) {
                out.push(oid);
            }
        }
        out
    }

    pub(super) fn set_attackers(
        &mut self,
        ids: &[u32],
        _player: PlayerId,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.priority_player_id() != _player {
            return Err(EngineError::Illegal("not your priority"));
        }
        let ap = self.state.active_player_id();

        // CR 508.1d: must-attack enforcement. A creature that must attack if able must be declared
        // as an attacker whenever it is a legal attacker. Same set the client is given via
        // LegalActions.required_attacker_ids, so the UI can gate its confirm control identically.
        for oid in self.required_attacker_ids() {
            if !ids.contains(&oid) {
                return Err(EngineError::Illegal(
                    "must-attack creature not declared as attacker",
                ));
            }
        }

        if ids.is_empty() {
            self.clear_all_mana_pools();
            self.state.combat = None;
            self.state.turn_step = TurnStep::EndCombat;
            if let Some(i) = self.state.player_idx(ap) {
                self.state.priority_idx = i;
            }
            self.state.passes_since_stack_change = 0;
            let mut b2 = RuledEventBatch::default();
            b2.events
                .push(ev_log("No attackers — skipped to end combat".to_string()));
            b2.events.push(ev_phase_labeled(self, "end_combat"));
            b2.events.push(ev_priority_changed(self));
            fill_legal(&mut b2, self);
            return Ok(b2);
        }
        let mut list = Vec::new();
        let mut seen_attackers = HashSet::new();
        for &oid in ids {
            if !seen_attackers.insert(oid) {
                return Err(EngineError::Illegal("duplicate attacker"));
            }
            let o = self
                .state
                .objects
                .get(&oid)
                .ok_or(EngineError::Illegal("attacker id"))?;
            if o.owner != ap || o.zone != Zone::Battlefield {
                return Err(EngineError::Illegal("illegal attacker"));
            }
            if !o.is_creature(self.registry) {
                return Err(EngineError::Illegal("not creature"));
            }
            // CR 702.3b: a creature with defender can't attack.
            if self.effective_has_keyword(oid, tricerules_cards::Keyword::Defender) {
                return Err(EngineError::Illegal("creature has defender"));
            }
            // CR 702.10b: Haste bypasses summoning sickness — the creature may attack even
            // if it entered the battlefield this turn.
            let has_haste = self.effective_has_keyword(oid, tricerules_cards::Keyword::Haste);
            if o.summoning_sick && !has_haste {
                return Err(EngineError::Illegal("summoning sick"));
            }
            if o.tapped {
                return Err(EngineError::Illegal("tapped"));
            }
            list.push(oid);
        }
        for &oid in &list {
            // CR 702.20b — Vigilance: attacking doesn't cause this creature to tap.
            let has_vigilance =
                self.effective_has_keyword(oid, tricerules_cards::Keyword::Vigilance);
            if !has_vigilance {
                if let Some(c) = self.state.objects.get_mut(&oid) {
                    c.tapped = true;
                }
            }
        }
        let attackers_for_event = list.clone();
        if let Some(c) = self.state.combat.as_mut() {
            c.attacking = list;
            c.blockers.clear();
            c.damage_assignments.clear();
            c.trample_player_damage.clear();
            c.damage_assignment_needed = false;
            c.assign_combat_damage_phase = false;
            c.attackers_declared = true;
            c.blockers_declared = false;
            c.first_strike_attackers.clear();
            c.first_strike_blockers.clear();
            c.first_strike_damage_done = false;
        } else {
            self.state.combat = Some(CombatState {
                attacking: list,
                blockers: HashMap::new(),
                damage_assignments: HashMap::new(),
                trample_player_damage: HashMap::new(),
                damage_assignment_needed: false,
                attackers_declared: true,
                blockers_declared: false,
                assign_combat_damage_phase: false,
                first_strike_attackers: Vec::new(),
                first_strike_blockers: HashMap::new(),
                first_strike_damage_done: false,
            });
        }
        self.clear_all_mana_pools();
        // MTG timing: after attackers are declared, the game remains in declare-attackers
        // and the active player receives priority before moving to declare blockers.
        self.state.turn_step = TurnStep::DeclareAttackers;
        if let Some(ai) = self.state.player_idx(ap) {
            self.state.priority_idx = ai;
        }
        self.state.passes_since_stack_change = 0;
        let mut b = RuledEventBatch::default();
        b.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::AttackersDeclared(
                rv1::AttackersDeclared {
                    attacking_player_id: ap,
                    attacker_object_ids: attackers_for_event.clone(),
                },
            )),
        });
        let atk_names: Vec<String> = attackers_for_event
            .iter()
            .map(|&oid| object_display_name(&self.state, self.registry, oid))
            .collect();
        b.events.push(ev_log(format!(
            "P{} attacks with {}",
            ap,
            atk_names.join(", ")
        )));
        self.fire_triggers(
            GameEvent::Attacks {
                attacker_ids: attackers_for_event,
            },
            &mut b.events,
        );
        b.events.push(ev_priority_changed(self));
        Ok(b)
    }

    pub(super) fn set_blockers(
        &mut self,
        pairs: &[rv1::BlockPair],
    ) -> Result<RuledEventBatch, EngineError> {
        let defending_player = self
            .state
            .defending_player_id_1v1()
            .ok_or(EngineError::Illegal("defender missing"))?;
        // A blocker may appear at most once: CR 509.1a — a creature can only block one attacker.
        let mut seen_blockers = HashSet::new();
        // Build attacker → [blockers] map while validating.
        let mut attacker_to_blockers: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();
        for p in pairs {
            let in_attack = self
                .state
                .combat
                .as_ref()
                .map(|c| c.attacking.contains(&p.attacker_id))
                .unwrap_or(false);
            if !in_attack {
                return Err(EngineError::Illegal("bad attacker"));
            }
            if !seen_blockers.insert(p.blocker_id) {
                return Err(EngineError::Illegal("blocker assigned more than once"));
            }
            let bobj = self
                .state
                .objects
                .get(&p.blocker_id)
                .ok_or(EngineError::Illegal("blocker?"))?;
            if bobj.zone != Zone::Battlefield {
                return Err(EngineError::Illegal("blocker zone"));
            }
            if bobj.owner != defending_player {
                return Err(EngineError::Illegal("not your blocker"));
            }
            if !bobj.is_creature(self.registry) {
                return Err(EngineError::Illegal("blocker not creature"));
            }
            if bobj.tapped {
                return Err(EngineError::Illegal("blocker tapped"));
            }
            // Evasion check: flying (CR 702.9b), intimidate (CR 702.13b), etc.
            if !self.can_block(p.attacker_id, p.blocker_id) {
                return Err(EngineError::Illegal(
                    "blocker cannot block this attacker (evasion)",
                ));
            }
            attacker_to_blockers
                .entry(p.attacker_id)
                .or_default()
                .push(p.blocker_id);
        }
        // CR 509.1c: must-block enforcement. A creature that must block if able must be declared
        // as a blocker whenever it is untapped and could legally block at least one attacker.
        // `seen_blockers` already holds every declared blocker; `required_blocker_ids` is the same
        // set surfaced to the client via LegalActions so the UI can gate its confirm control.
        for oid in self.required_blocker_ids() {
            if !seen_blockers.contains(&oid) {
                return Err(EngineError::Illegal(
                    "must-block creature not declared as blocker",
                ));
            }
        }

        // CR 702.111: menace — a creature with menace can't be blocked except by two or more
        // creatures. A menace creature with zero blockers is fine (it's unblocked); one blocker
        // is the illegal case. Return a prompt-friendly message so the UI can surface it.
        for (&att_id, blk_ids) in &attacker_to_blockers {
            if blk_ids.len() < 2 {
                let has_menace =
                    self.effective_has_keyword(att_id, tricerules_cards::Keyword::Menace);
                if has_menace {
                    return Err(EngineError::Illegal("Illegal blocks."));
                }
            }
        }
        // CR 702.19: trample attackers with 1+ blockers also require explicit damage assignment
        // (to split damage between blockers and the defending player).
        let damage_assignment_needed = attacker_to_blockers.iter().any(|(atk_id, blks)| {
            let has_trample =
                self.effective_has_keyword(*atk_id, tricerules_cards::Keyword::Trample);
            blks.len() > 1 || (blks.len() == 1 && has_trample)
        });
        if let Some(c) = self.state.combat.as_mut() {
            c.blockers = attacker_to_blockers;
            c.damage_assignments.clear();
            c.trample_player_damage.clear();
            c.damage_assignment_needed = damage_assignment_needed;
            c.assign_combat_damage_phase = false;
            c.blockers_declared = true;
        }
        let block_line = if pairs.is_empty() {
            "declares no blockers".to_string()
        } else {
            pairs
                .iter()
                .map(|p| {
                    let att = object_display_name(&self.state, self.registry, p.attacker_id);
                    let blk = object_display_name(&self.state, self.registry, p.blocker_id);
                    format!("{blk} blocks {att}")
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let mut b = RuledEventBatch::default();
        let block_pairs_for_event: Vec<rv1::BlockPair> = pairs.to_vec();
        b.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::BlockersDeclared(
                rv1::BlockersDeclared {
                    block_pairs: block_pairs_for_event,
                },
            )),
        });
        self.clear_all_mana_pools();
        // MTG timing: blockers are declared in declare-blockers, then players get priority
        // before the game advances into combat-damage where damage is actually dealt.
        self.state.turn_step = TurnStep::DeclareBlockers;
        if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
            self.state.priority_idx = i;
        }
        self.state.passes_since_stack_change = 0;
        b.events
            .push(ev_log(format!("P{} {}", defending_player, block_line)));
        b.events.push(ev_priority_changed(self));
        fill_legal(&mut b, self);
        Ok(b)
    }

    pub(super) fn assign_combat_damage(
        &mut self,
        attacker_id: ObjectId,
        assignments: &[(ObjectId, u32)],
        player_damage: u32,
    ) -> Result<RuledEventBatch, EngineError> {
        // Phase 1: check gating conditions (immutable borrow, dropped at end of block).
        {
            let c = self
                .state
                .combat
                .as_ref()
                .ok_or(EngineError::Illegal("not in combat"))?;
            if !c.blockers_declared || !c.damage_assignment_needed || !c.assign_combat_damage_phase
            {
                return Err(EngineError::Illegal("combat damage assignment not open"));
            }
        }

        // Phase 2: compute trample flag and expected blockers before any borrow of combat.
        let att_has_trample =
            self.effective_has_keyword(attacker_id, tricerules_cards::Keyword::Trample);
        // CR 702.2b: any nonzero damage from a deathtouch source is lethal, which lowers the
        // per-blocker lethal amount the trample assignment must cover (CR 702.19e) to 1.
        let att_has_deathtouch =
            self.effective_has_keyword(attacker_id, tricerules_cards::Keyword::Deathtouch);

        // Clone expected blockers to free the immutable borrow on combat before the mutable one.
        let expected_blockers: Vec<ObjectId> = self
            .state
            .combat
            .as_ref()
            .and_then(|c| c.blockers.get(&attacker_id))
            .ok_or(EngineError::Illegal("attacker not blocked"))?
            .clone();

        if expected_blockers.len() < 2 && !att_has_trample {
            return Err(EngineError::Illegal("attacker not multiply-blocked"));
        }
        if expected_blockers.is_empty() {
            return Err(EngineError::Illegal(
                "cannot assign damage for unblocked attacker",
            ));
        }

        // Phase 3: validate assignment set (all expected blockers exactly once).
        let mut seen_block = HashSet::new();
        for &(bid, _) in assignments {
            if !seen_block.insert(bid) {
                return Err(EngineError::Illegal("duplicate blocker in assignments"));
            }
        }
        let provided: HashSet<ObjectId> = assignments.iter().map(|(b, _)| *b).collect();
        let expected_set: HashSet<ObjectId> = expected_blockers.iter().copied().collect();
        if provided != expected_set {
            return Err(EngineError::Illegal(
                "assignments must list each blocker exactly once",
            ));
        }

        let att_power = self
            .effective_power(attacker_id)
            .ok_or(EngineError::Illegal("attacker missing"))?;

        // Phase 4: validate damage amounts per trample rules.
        if att_has_trample {
            // CR 702.19b: must assign >= lethal damage to each blocker before sending excess to player.
            for &blk in &expected_blockers {
                let blk_toughness = self.effective_toughness(blk).unwrap_or(1);
                let marked = self.state.objects.get(&blk).map(|o| o.damage).unwrap_or(0);
                // CR 702.19e: with deathtouch, 1 damage counts as lethal for assignment, so the
                // attacker may assign just 1 to each blocker before trampling the rest over.
                let lethal = if att_has_deathtouch {
                    1
                } else {
                    blk_toughness.saturating_sub(marked).max(1)
                };
                let assigned = assignments
                    .iter()
                    .find(|(b, _)| *b == blk)
                    .map(|(_, d)| *d)
                    .unwrap_or(0);
                if assigned < lethal {
                    return Err(EngineError::Illegal(
                        "trample: must assign lethal damage to each blocker before assigning to player",
                    ));
                }
            }
            let blocker_sum: u32 = assignments.iter().map(|(_, d)| d).sum();
            if blocker_sum + player_damage != att_power {
                return Err(EngineError::Illegal(
                    "trample: total damage (blockers + player) must equal attacker power",
                ));
            }
        } else {
            if player_damage != 0 {
                return Err(EngineError::Illegal(
                    "cannot assign player damage without trample",
                ));
            }
            // CR 510.1c (post-2017): a multiply-blocked attacker no longer uses a declared
            // "damage assignment order" — the attacking player now freely divides the attacker's
            // combat damage among its blockers. So the only constraint here is that the assigned
            // amounts sum to the attacker's power; any per-blocker split is legal. (This is the
            // current rule, NOT a simplification — do not re-add an ordering/lethal-first check.)
            let sum: u32 = assignments.iter().map(|(_, d)| d).sum();
            if sum != att_power {
                return Err(EngineError::Illegal(
                    "assigned damage must equal attacker power",
                ));
            }
        }

        // Phase 5: store the assignment and check completion (mutable borrow).
        // Pre-compute which attackers need assignment to avoid borrowing self inside the closure.
        let needs_assignment: Vec<ObjectId> = self
            .state
            .combat
            .as_ref()
            .unwrap()
            .blockers
            .iter()
            .filter_map(|(atk_id, blks)| {
                let has_trample =
                    self.effective_has_keyword(*atk_id, tricerules_cards::Keyword::Trample);
                if blks.len() > 1 || (blks.len() == 1 && has_trample) {
                    Some(*atk_id)
                } else {
                    None
                }
            })
            .collect();

        let mut b = RuledEventBatch::default();
        let c = self.state.combat.as_mut().unwrap();
        c.damage_assignments
            .insert(attacker_id, assignments.to_vec());
        if att_has_trample && player_damage > 0 {
            c.trample_player_damage.insert(attacker_id, player_damage);
        }
        let all_done = needs_assignment
            .iter()
            .all(|atk| c.damage_assignments.contains_key(atk));
        if all_done {
            c.damage_assignment_needed = false;
        }
        let proto_pairs: Vec<rv1::DamagePair> = assignments
            .iter()
            .map(|&(bid, dmg)| rv1::DamagePair {
                blocker_id: bid,
                damage: dmg,
            })
            .collect();
        b.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::CombatDamageAssigned(
                rv1::CombatDamageAssigned {
                    attacker_id,
                    assignments: proto_pairs,
                },
            )),
        });
        let att_name = object_display_name(&self.state, self.registry, attacker_id);
        b.events
            .push(ev_log(format!("Combat damage assigned for {att_name}.")));
        if !self.state.combat.as_ref().unwrap().damage_assignment_needed {
            self.resolve_combat_damage_step(&mut b.events)?;
        } else {
            b.events.push(ev_priority_changed(self));
        }
        fill_legal(&mut b, self);
        Ok(b)
    }

    /// Resolve the current combat damage step (CR 510). Routes through the first-strike
    /// substep when any combatant has FirstStrike/DoubleStrike, then through the regular
    /// damage step. Emits phase labels, applies SBAs, and updates priority. Both call sites
    /// (the post-`assign_combat_damage` path and the `DeclareBlockers → CombatDamage` pass)
    /// go through this helper so the logic stays in one place.
    pub(super) fn resolve_combat_damage_step(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        use tricerules_cards::Keyword;
        let ap = self.state.active_player_id();
        let c_init = self
            .state
            .combat
            .clone()
            .ok_or(EngineError::Illegal("combat?"))?;
        let needs_first_strike =
            !c_init.first_strike_damage_done && combat_needs_first_strike_step(self, &c_init);

        if needs_first_strike {
            // Snapshot which creatures had FS/DS at the start of the first-strike step. This is
            // the canonical CR 510.4 "participation list" used to exclude them from the regular
            // step (unless they have DoubleStrike).
            let is_fs_or_ds = |id: ObjectId| {
                self.effective_has_keyword(id, Keyword::FirstStrike)
                    || self.effective_has_keyword(id, Keyword::DoubleStrike)
            };
            let fs_attackers: Vec<ObjectId> = c_init
                .attacking
                .iter()
                .copied()
                .filter(|&id| is_fs_or_ds(id))
                .collect();
            let fs_blockers: HashMap<ObjectId, Vec<ObjectId>> = c_init
                .blockers
                .iter()
                .map(|(att, bs)| {
                    (
                        *att,
                        bs.iter().copied().filter(|&id| is_fs_or_ds(id)).collect(),
                    )
                })
                .collect();
            if let Some(cc) = self.state.combat.as_mut() {
                cc.first_strike_attackers = fs_attackers;
                cc.first_strike_blockers = fs_blockers;
                cc.first_strike_damage_done = true;
            }
            let c2 = self
                .state
                .combat
                .clone()
                .ok_or(EngineError::Illegal("combat?"))?;
            // Emit PhaseChanged before resolving damage so the C++ client clears its
            // stack-object set before any combat damage triggers are pushed (StackPushed).
            // This mirrors adv_on_empty_stack(Untap) which emits PhaseChanged first, then
            // fires upkeep triggers — ensuring players see the non-empty stack and are not
            // auto-passed through triggered abilities.
            self.clear_all_mana_pools();
            self.state.turn_step = TurnStep::FirstStrikeDamage;
            if let Some(i) = self.state.player_idx(ap) {
                self.state.priority_idx = i;
            }
            self.state.passes_since_stack_change = 0;
            events.push(ev_log("First strike combat damage dealt.".to_string()));
            events.push(ev_phase_labeled(self, "first_strike_damage"));
            self.resolve_combat_damage(&c2, DamagePass::FirstStrike, events)?;
            // CR 510.2 + 704: SBAs run between damage steps so creatures with lethal damage are
            // moved to graveyards before the regular step decides who deals damage.
            self.apply_sbas(events)?;
            let legend_events = self.apply_legend_sbas()?;
            events.extend(legend_events);
            events.push(ev_priority_changed(self));
        } else {
            // Emit PhaseChanged before resolving damage so the C++ client clears its
            // stack-object set before any combat damage triggers are pushed (StackPushed).
            self.state.combat = None;
            self.clear_all_mana_pools();
            self.state.turn_step = TurnStep::CombatDamage;
            if let Some(i) = self.state.player_idx(ap) {
                self.state.priority_idx = i;
            }
            self.state.passes_since_stack_change = 0;
            events.push(ev_log("Combat damage dealt.".to_string()));
            events.push(ev_phase_labeled(self, "combat_damage"));
            self.resolve_combat_damage(&c_init, DamagePass::Normal, events)?;
            let legend_events = self.apply_legend_sbas()?;
            events.extend(legend_events);
            events.push(ev_priority_changed(self));
        }
        Ok(())
    }

    pub(super) fn resolve_combat_damage(
        &mut self,
        c: &CombatState,
        pass: DamagePass,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        use tricerules_cards::Keyword;
        let dfd = self.state.defending_player_id_1v1().unwrap();
        let ap = self.state.active_player_id();
        let mut total_life_lost: i32 = 0;
        // (controller_id, amount) pairs — collected during damage assignment, applied after.
        let mut lifelink_gains: Vec<(PlayerId, u32)> = Vec::new();
        // (attacker_id, defending_player_id) — collected for combat-damage-to-player triggers.
        let mut combat_dmg_to_player: Vec<(ObjectId, PlayerId)> = Vec::new();

        // CR 510.4 ASSIGNMENT rule: in the first-strike pass, only creatures with FirstStrike
        // or DoubleStrike assign damage; in the regular pass, creatures that did NOT assign
        // in the first-strike pass do, plus those that have DoubleStrike. Crucially, creatures
        // RECEIVE damage normally regardless of *their own* participation — a vanilla blocker
        // can be killed by a first-strike attacker before it ever swings, and a vanilla blocker
        // still deals damage back to a first-strike attacker in the regular step. We therefore
        // iterate over ALL attackers and gate each damage direction independently:
        //   - "attacker deals damage" -> attacker's participation
        //   - "blocker deals damage" -> blocker's participation
        // When no first-strike step occurred (`c.first_strike_attackers` empty), every creature
        // participates in the regular pass (vanilla combat).

        for &att in &c.attacking {
            if self.state.objects.get(&att).map(|a| a.zone) != Some(Zone::Battlefield) {
                continue;
            }
            let attacker_participates = object_participates_in_pass(self, c, pass, att, true);
            // Capture attacker properties before any mutation.
            let att_power = self.effective_power(att).unwrap_or(0);
            let att_has_lifelink = self.effective_has_keyword(att, Keyword::Lifelink);
            let att_has_deathtouch = self.effective_has_keyword(att, Keyword::Deathtouch);
            let att_owner = self.state.objects.get(&att).map(|o| o.owner).unwrap_or(ap);
            let att_has_trample = self.effective_has_keyword(att, Keyword::Trample);

            let blockers = c.blockers.get(&att).map(|v| v.as_slice()).unwrap_or(&[]);

            if blockers.is_empty() {
                // Unblocked: deal full power to defending player — only if the attacker assigns
                // damage this pass (CR 510.4).
                if attacker_participates {
                    let p = att_power as i32;
                    if let Some(di) = self.state.player_idx(dfd) {
                        self.state.players[di].life -= p;
                        total_life_lost += p;
                    }
                    if att_power > 0 {
                        combat_dmg_to_player.push((att, dfd));
                    }
                    // CR 702.15b: attacker with lifelink causes its controller to gain that much life.
                    if att_has_lifelink && att_power > 0 {
                        lifelink_gains.push((att_owner, att_power));
                    }
                }
            } else if blockers.len() == 1 && !att_has_trample {
                // Single blocker, no trample: exchange power. The attacker always deals damage to
                // its sole blocker (since we're in the attacker's participation loop), but the
                // blocker only deals damage back if it participates in this pass (CR 510.4).
                let blk = blockers[0];
                let blocker_participates = object_participates_in_pass(self, c, pass, blk, false)
                    && self.state.objects.get(&blk).map(|o| o.zone) == Some(Zone::Battlefield);
                let bpw = self.effective_power(blk).unwrap_or(0);
                let blk_has_lifelink = self.effective_has_keyword(blk, Keyword::Lifelink);
                let blk_has_deathtouch = self.effective_has_keyword(blk, Keyword::Deathtouch);
                let blk_owner = self.state.objects.get(&blk).map(|o| o.owner).unwrap_or(dfd);
                if blocker_participates {
                    if let Some(af) = self.state.objects.get_mut(&att) {
                        af.damage += bpw;
                        // CR 702.2b / CR 704.5h: any damage from a deathtouch source is lethal.
                        if blk_has_deathtouch && bpw > 0 {
                            af.deathtouch_damage = true;
                        }
                    }
                }
                if attacker_participates {
                    if let Some(bf) = self.state.objects.get_mut(&blk) {
                        bf.damage += att_power;
                        // CR 702.2b: any damage from attacker with deathtouch is lethal.
                        if att_has_deathtouch && att_power > 0 {
                            bf.deathtouch_damage = true;
                        }
                    }
                    // CR 702.15b: attacker with lifelink gains life = damage dealt to blocker.
                    if att_has_lifelink && att_power > 0 {
                        lifelink_gains.push((att_owner, att_power));
                    }
                }
                // CR 702.15b: blocker with lifelink gains life = damage dealt to attacker.
                if blocker_participates && blk_has_lifelink && bpw > 0 {
                    lifelink_gains.push((blk_owner, bpw));
                }
            } else {
                // Multiple blockers OR single-blocker with trample: all blockers deal their power
                // to the attacker simultaneously; active player assigns how the attacker's combat
                // damage is divided among blockers (and, for trample, the defending player).
                // CR 510.4: in a given damage step, only participating blockers deal damage back.
                // Tuple: (id, power, has_lifelink, has_deathtouch, owner, participates)
                let blocker_info: Vec<(ObjectId, u32, bool, bool, PlayerId, bool)> = blockers
                    .iter()
                    .map(|&blk| {
                        let pw = self.effective_power(blk).unwrap_or(0);
                        let has_ll = self.effective_has_keyword(blk, Keyword::Lifelink);
                        let has_dt = self.effective_has_keyword(blk, Keyword::Deathtouch);
                        let owner = self.state.objects.get(&blk).map(|o| o.owner).unwrap_or(dfd);
                        let participates = object_participates_in_pass(self, c, pass, blk, false)
                            && self.state.objects.get(&blk).map(|o| o.zone)
                                == Some(Zone::Battlefield);
                        (blk, pw, has_ll, has_dt, owner, participates)
                    })
                    .collect();
                let total_blocker_power: u32 = blocker_info
                    .iter()
                    .filter(|(_, _, _, _, _, p)| *p)
                    .map(|(_, pw, _, _, _, _)| pw)
                    .sum();
                // CR 702.2b: if any participating blocker has deathtouch and dealt damage,
                // mark the attacker.
                let any_blocker_deathtouch_hit = blocker_info
                    .iter()
                    .any(|(_, pw, _, has_dt, _, p)| *p && *has_dt && *pw > 0);
                if let Some(af) = self.state.objects.get_mut(&att) {
                    af.damage += total_blocker_power;
                    if any_blocker_deathtouch_hit {
                        af.deathtouch_damage = true;
                    }
                }
                // The attacker assigns damage to its blockers only on a pass it participates in
                // (CR 510.4). On the off pass, blockers still deal damage back (handled above).
                if attacker_participates {
                    let pairs = c.damage_assignments.get(&att).ok_or(EngineError::Illegal(
                        "combat damage assignments missing for multiply-blocked attacker",
                    ))?;
                    for &(blk, dmg) in pairs {
                        if let Some(bf) = self.state.objects.get_mut(&blk) {
                            bf.damage += dmg;
                            // CR 702.2b: any damage from attacker with deathtouch is lethal.
                            if att_has_deathtouch && dmg > 0 {
                                bf.deathtouch_damage = true;
                            }
                        }
                    }
                    // CR 702.19: deal trample excess damage to the defending player.
                    let player_trample_dmg =
                        c.trample_player_damage.get(&att).copied().unwrap_or(0);
                    if player_trample_dmg > 0 {
                        if let Some(di) = self.state.player_idx(dfd) {
                            self.state.players[di].life -= player_trample_dmg as i32;
                            total_life_lost += player_trample_dmg as i32;
                        }
                        // CR 510: trample excess is combat damage the attacker deals to the
                        // defending player, so it fires "deals combat damage to a player" triggers
                        // exactly like an unblocked hit.
                        combat_dmg_to_player.push((att, dfd));
                    }
                    // CR 702.15b: attacker with lifelink gains life = damage dealt to all blockers.
                    if att_has_lifelink && att_power > 0 {
                        lifelink_gains.push((att_owner, att_power));
                    }
                }
                // CR 702.15b: each participating blocker with lifelink gains life = damage it dealt
                // to the attacker.
                for (_, blk_pw, blk_has_ll, _, blk_owner, blk_participates) in blocker_info {
                    if blk_participates && blk_has_ll && blk_pw > 0 {
                        lifelink_gains.push((blk_owner, blk_pw));
                    }
                }
            }
        }
        if total_life_lost > 0 {
            if let Some(di) = self.state.player_idx(dfd) {
                let new_total = self.state.players[di].life;
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                        player_id: dfd,
                        new_total,
                        delta: -total_life_lost,
                    })),
                });
            }
        }
        // Apply lifelink gains and emit LifeChanged events.
        for (pid, amount) in lifelink_gains {
            if let Some(pi) = self.state.player_idx(pid) {
                self.state.players[pi].life += amount as i32;
                let new_total = self.state.players[pi].life;
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                        player_id: pid,
                        new_total,
                        delta: amount as i32,
                    })),
                });
                events.push(ev_log(format!("P{pid} gains {amount} life (lifelink).")));
            }
        }
        for (att_id, def_id) in combat_dmg_to_player {
            self.fire_triggers(
                GameEvent::CombatDamageToPlayer {
                    attacker_id: att_id,
                    defender_id: def_id,
                },
                events,
            );
        }
        Ok(())
    }
}

/// True while the game is waiting for attack or block declarations before
/// players may take spell/activated actions that require priority (CR 508 / 509).
pub(super) fn priority_locked_for_combat_declaration(state: &GameState) -> bool {
    match state.turn_step {
        TurnStep::DeclareAttackers => state.combat.as_ref().is_some_and(|c| !c.attackers_declared),
        TurnStep::DeclareBlockers => state.combat.as_ref().is_some_and(|c| !c.blockers_declared),
        _ => false,
    }
}

/// Which combat damage step is being resolved (CR 510.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DamagePass {
    FirstStrike,
    Normal,
}

/// CR 510.4 participation rule. In the first-strike pass, only creatures with FirstStrike or
/// DoubleStrike assign damage. In the regular pass, creatures that did not assign during the
/// first-strike step (or weren't in it) assign damage, plus creatures that currently have
/// DoubleStrike. When no first-strike step occurred, every creature participates in the
/// regular pass (vanilla combat).
pub(super) fn object_participates_in_pass(
    engine: &GameEngine,
    c: &CombatState,
    pass: DamagePass,
    obj_id: ObjectId,
    is_attacker: bool,
) -> bool {
    use tricerules_cards::Keyword;
    let has_fs = engine.effective_has_keyword(obj_id, Keyword::FirstStrike);
    let has_ds = engine.effective_has_keyword(obj_id, Keyword::DoubleStrike);
    match pass {
        DamagePass::FirstStrike => has_fs || has_ds,
        DamagePass::Normal => {
            let was_in_first_strike = if is_attacker {
                c.first_strike_attackers.contains(&obj_id)
            } else {
                c.first_strike_blockers
                    .values()
                    .any(|bs| bs.contains(&obj_id))
            };
            !was_in_first_strike || has_ds
        }
    }
}

/// True iff any current attacker or blocker has FirstStrike or DoubleStrike — used to decide
/// whether the combat phase needs a first-strike damage substep (CR 510.4).
pub(super) fn combat_needs_first_strike_step(engine: &GameEngine, c: &CombatState) -> bool {
    use tricerules_cards::Keyword;
    let has_fs_or_ds = |id: ObjectId| {
        engine.effective_has_keyword(id, Keyword::FirstStrike)
            || engine.effective_has_keyword(id, Keyword::DoubleStrike)
    };
    c.attacking.iter().copied().any(has_fs_or_ds)
        || c.blockers.values().flatten().copied().any(has_fs_or_ds)
}

/// CR 508/509: true if `oid` is currently an attacker or a blocker in the active combat.
pub(super) fn is_attacking_or_blocking(state: &GameState, oid: ObjectId) -> bool {
    let Some(combat) = &state.combat else {
        return false;
    };
    combat.attacking.contains(&oid) || combat.blockers.values().any(|bs| bs.contains(&oid))
}
