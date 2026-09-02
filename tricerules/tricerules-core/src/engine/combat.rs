mod blocking;
use blocking::BlockGraph;

use super::damage::{DamageEvent, DamageRecipient};
use super::events::{ev_log, ev_phase, ev_priority_changed, object_display_name};
use super::legal_actions::fill_legal;
use super::*;

impl GameEngine {
    /// CR 702.190a: the object returned for Sneak must still be an unblocked attacking creature
    /// controlled by the caster in that caster's declare-blockers step. The blocker map retains
    /// an attacker key after its last blocker leaves, so key absence is the authoritative
    /// "unblocked" test rather than an empty current blocker list.
    pub(super) fn sneak_return_assignment(
        &self,
        player: PlayerId,
        object: &rv1::CostObjectRef,
    ) -> Option<CombatAttackAssignment> {
        if self.state.active_player_id() != player
            || self.state.priority_player_id() != player
            || self.state.turn_step != TurnStep::DeclareBlockers
        {
            return None;
        }
        let combat = self.state.combat.as_ref()?;
        if !combat.blockers_declared
            || !combat.attacking.contains(&object.object_id)
            || combat.blockers.contains_key(&object.object_id)
        {
            return None;
        }
        let assignment = *combat.attack_assignments.get(&object.object_id)?;
        let current_generation = self
            .state
            .zone_change_generation
            .get(&object.object_id)
            .copied()
            .unwrap_or(0);
        let permanent = self.state.objects.get(&object.object_id)?;
        (current_generation == object.zone_change_generation
            && assignment.attacker.zone_change_generation == current_generation
            && permanent.zone == Zone::Battlefield
            && permanent.controller == player
            && self
                .characteristics(object.object_id)
                .is_some_and(|characteristics| characteristics.is_creature()))
        .then_some(assignment)
    }

    pub(super) fn sneak_return_candidates(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .combat
            .as_ref()
            .map(|combat| combat.attacking.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|object_id| {
                self.sneak_return_assignment(
                    player,
                    &rv1::CostObjectRef {
                        object_id: *object_id,
                        zone_change_generation: self
                            .state
                            .zone_change_generation
                            .get(object_id)
                            .copied()
                            .unwrap_or(0),
                    },
                )
                .is_some()
            })
            .collect()
    }

    /// CR 702.190b / 506.3: a resolving Sneak permanent inherits the paid creature's recipient,
    /// but it was never declared as an attacker. Only the current entrant and captured recipient
    /// are revalidated; declaration restrictions are intentionally not applied.
    pub(super) fn add_sneak_attacker(
        &mut self,
        object_id: ObjectId,
        paid_assignment: CombatAttackAssignment,
    ) -> Option<rv1::AttackAssignment> {
        let object = self.state.objects.get(&object_id)?;
        if object.zone != Zone::Battlefield
            || object.controller != self.state.active_player_id()
            || !self
                .characteristics(object_id)
                .is_some_and(|characteristics| characteristics.is_creature())
        {
            return None;
        }
        let defending_player_valid = self
            .state
            .players
            .iter()
            .any(|player| player.id == paid_assignment.defending_player && !player.has_lost);
        let defender_valid = match paid_assignment.defender {
            CombatDefenderTarget::Player(player) => self
                .state
                .players
                .iter()
                .any(|candidate| candidate.id == player && !candidate.has_lost),
            CombatDefenderTarget::Permanent(defender) => self
                .state
                .objects
                .get(&defender.object_id)
                .is_some_and(|permanent| {
                    permanent.zone == Zone::Battlefield
                        && self
                            .state
                            .zone_change_generation
                            .get(&defender.object_id)
                            .copied()
                            .unwrap_or(0)
                            == defender.zone_change_generation
                        && self
                            .characteristics(defender.object_id)
                            .is_some_and(|values| {
                                values.has_type("Planeswalker") || values.has_type("Battle")
                            })
                }),
        };
        if !defending_player_valid
            || !defender_valid
            || self
                .state
                .combat
                .as_ref()
                .is_none_or(|combat| !combat.attackers_declared)
        {
            return None;
        }
        let attacker = self.trigger_object_ref(object_id)?;
        let assignment = CombatAttackAssignment {
            attacker,
            defender: paid_assignment.defender,
            defending_player: paid_assignment.defending_player,
        };
        let wire = self.wire_attack_assignment(
            object_id,
            assignment.defender,
            assignment.defending_player,
        );
        let combat = self.state.combat.as_mut()?;
        combat.attacking.push(object_id);
        combat.attack_assignments.insert(object_id, assignment);
        Some(wire)
    }

    pub(super) fn combat_defender_recipient(
        &self,
        combat: &CombatState,
        attacker: ObjectId,
    ) -> Option<DamageRecipient> {
        let assignment = combat.attack_assignments.get(&attacker)?;
        match assignment.defender {
            CombatDefenderTarget::Player(player) => self
                .state
                .player_idx(player)
                .is_some()
                .then_some(DamageRecipient::Player(player)),
            CombatDefenderTarget::Permanent(permanent) => {
                let generation = self
                    .state
                    .zone_change_generation
                    .get(&permanent.object_id)
                    .copied()
                    .unwrap_or(0);
                self.state
                    .objects
                    .get(&permanent.object_id)
                    .is_some_and(|object| object.zone == Zone::Battlefield)
                    .then_some(())
                    .filter(|_| generation == permanent.zone_change_generation)
                    .map(|_| DamageRecipient::Permanent(permanent.object_id))
            }
        }
    }

    /// Returns whether an attacker needs an explicit combat-damage assignment: either it has
    /// multiple blockers, or it has one blocker and trample can carry excess damage to the player.
    pub(super) fn attacker_needs_explicit_damage_assignment(
        &self,
        attacker_id: ObjectId,
        blocker_count: usize,
    ) -> bool {
        blocker_count > 1
            || (blocker_count == 1
                && self.effective_has_keyword(attacker_id, tricerules_cards::Keyword::Trample))
    }

    fn combat_restrictions(&self, oid: ObjectId) -> CombatRestriction {
        let Some(characteristics) = self.characteristics(oid) else {
            return CombatRestriction::default();
        };
        self.combat_restrictions_for(oid, &characteristics)
    }

    pub(super) fn combat_restrictions_for(
        &self,
        oid: ObjectId,
        characteristics: &super::characteristics::Characteristics,
    ) -> CombatRestriction {
        self.state
            .continuous_effects
            .iter()
            .filter(|effect| {
                super::characteristics::effect_affects(
                    &self.state,
                    self.registry,
                    effect,
                    oid,
                    characteristics,
                )
            })
            .filter(|effect| self.continuous_effect_condition_holds(effect))
            .fold(CombatRestriction::default(), |mut combined, effect| {
                if let ContinuousEffectKind::CombatRestriction(restriction) = &effect.kind {
                    combined.combine(restriction);
                }
                combined
            })
    }

    fn can_attack_as_though_without_defender(
        &self,
        oid: ObjectId,
        characteristics: &super::characteristics::Characteristics,
    ) -> bool {
        self.state.continuous_effects.iter().any(|effect| {
            matches!(
                effect.kind,
                ContinuousEffectKind::AttackAsThoughWithoutDefender
            ) && super::characteristics::effect_affects(
                &self.state,
                self.registry,
                effect,
                oid,
                characteristics,
            ) && self.continuous_effect_condition_holds(effect)
        })
    }

    fn attacker_illegality(&self, oid: ObjectId, active_player: PlayerId) -> Option<&'static str> {
        let Some(object) = self.state.objects.get(&oid) else {
            return Some("attacker id");
        };
        if object.zone != Zone::Battlefield {
            return Some("illegal attacker");
        }
        let Some(characteristics) = self.characteristics(oid) else {
            return Some("attacker id");
        };
        if characteristics.controller != active_player {
            return Some("illegal attacker");
        }
        if !characteristics.is_creature() {
            return Some("not creature");
        }
        if characteristics.has_keyword(tricerules_cards::Keyword::Defender)
            && !self.can_attack_as_though_without_defender(oid, &characteristics)
        {
            return Some("creature has defender");
        }
        if self.combat_restrictions(oid).cant_attack {
            return Some("creature cannot attack");
        }
        if object.summoning_sick && !characteristics.has_keyword(tricerules_cards::Keyword::Haste) {
            return Some("summoning sick");
        }
        if object.tapped {
            return Some("tapped");
        }
        None
    }

    pub(super) fn eligible_attacker_ids(&self, player: PlayerId) -> Vec<ObjectId> {
        let Some(player_idx) = self.state.player_idx(player) else {
            return Vec::new();
        };
        self.state.players[player_idx]
            .battlefield
            .iter()
            .copied()
            .filter(|oid| self.attacker_illegality(*oid, player).is_none())
            .collect()
    }

    fn base_blocker_eligible(&self, oid: ObjectId, defending_player: PlayerId) -> bool {
        let Some(object) = self.state.objects.get(&oid) else {
            return false;
        };
        object.zone == Zone::Battlefield
            && !object.tapped
            && self.characteristics(oid).is_some_and(|characteristics| {
                characteristics.controller == defending_player && characteristics.is_creature()
            })
    }

    pub(super) fn attack_defenders(&self) -> Vec<(CombatDefenderTarget, PlayerId)> {
        let defending_players = self.state.defending_player_ids();
        let mut defenders: Vec<_> = defending_players
            .iter()
            .copied()
            .map(|player| (CombatDefenderTarget::Player(player), player))
            .collect();
        for object in self.state.objects.values() {
            if object.zone != Zone::Battlefield {
                continue;
            }
            let Some(characteristics) = self.characteristics(object.id) else {
                continue;
            };
            let defending_player = if characteristics.has_type("Planeswalker")
                && defending_players.contains(&characteristics.controller)
            {
                Some(characteristics.controller)
            } else if characteristics.has_type("Battle") {
                self.state
                    .battle_protectors
                    .get(&object.id)
                    .copied()
                    .filter(|player| defending_players.contains(player))
            } else {
                None
            };
            let Some(defending_player) = defending_player else {
                continue;
            };
            defenders.push((
                CombatDefenderTarget::Permanent(TriggerObjectRef {
                    object_id: object.id,
                    zone_change_generation: self
                        .state
                        .zone_change_generation
                        .get(&object.id)
                        .copied()
                        .unwrap_or(0),
                    controller_at_event: characteristics.controller,
                }),
                defending_player,
            ));
        }
        defenders
    }

    pub(super) fn wire_attack_assignment(
        &self,
        attacker: ObjectId,
        defender: CombatDefenderTarget,
        defending_player: PlayerId,
    ) -> rv1::AttackAssignment {
        let (defender, defender_zone_change_generation) = match defender {
            CombatDefenderTarget::Player(player) => (
                rv1::TargetRef {
                    object_id: player as u32,
                    kind: rv1::TargetRefKind::Player as i32,
                    ..Default::default()
                },
                0,
            ),
            CombatDefenderTarget::Permanent(permanent) => (
                rv1::TargetRef {
                    object_id: permanent.object_id,
                    kind: rv1::TargetRefKind::Permanent as i32,
                    ..Default::default()
                },
                permanent.zone_change_generation,
            ),
        };
        rv1::AttackAssignment {
            attacker_object_id: attacker,
            attacker_zone_change_generation: self
                .state
                .zone_change_generation
                .get(&attacker)
                .copied()
                .unwrap_or(0),
            defender: Some(defender),
            defender_zone_change_generation,
            defending_player_id: defending_player,
        }
    }

    pub(super) fn legal_combat_defender_options(&self) -> Vec<rv1::CombatDefenderOption> {
        self.attack_defenders()
            .into_iter()
            .map(|(defender, defending_player)| {
                let assignment = self.wire_attack_assignment(0, defender, defending_player);
                rv1::CombatDefenderOption {
                    defender: assignment.defender,
                    defender_zone_change_generation: assignment.defender_zone_change_generation,
                    defending_player_id: assignment.defending_player_id,
                }
            })
            .collect()
    }

    pub(super) fn add_attacking_objects(
        &mut self,
        object_ids: &[ObjectId],
        options: &[rv1::CombatDefenderOption],
    ) -> Result<Vec<rv1::AttackAssignment>, EngineError> {
        if object_ids.len() != options.len() {
            return Err(EngineError::Illegal(
                "attacking-token defender count mismatch",
            ));
        }
        let legal_options = self.legal_combat_defender_options();
        let mut parsed = Vec::with_capacity(object_ids.len());
        for (&object_id, option) in object_ids.iter().zip(options) {
            if !legal_options.contains(option) {
                return Err(EngineError::Illegal(
                    "illegal or stale attacking-token defender",
                ));
            }
            let defender_ref = option
                .defender
                .as_ref()
                .ok_or(EngineError::Illegal("attacking-token defender missing"))?;
            let defender = match rv1::TargetRefKind::try_from(defender_ref.kind) {
                Ok(rv1::TargetRefKind::Player) => {
                    CombatDefenderTarget::Player(defender_ref.object_id as PlayerId)
                }
                Ok(rv1::TargetRefKind::Permanent) => {
                    CombatDefenderTarget::Permanent(TriggerObjectRef {
                        object_id: defender_ref.object_id,
                        zone_change_generation: option.defender_zone_change_generation,
                        controller_at_event: self
                            .state
                            .objects
                            .get(&defender_ref.object_id)
                            .map(|object| object.controller)
                            .ok_or(EngineError::Illegal("attacking-token defender missing"))?,
                    })
                }
                _ => {
                    return Err(EngineError::Illegal(
                        "invalid attacking-token defender kind",
                    ))
                }
            };
            let attacker = TriggerObjectRef {
                object_id,
                zone_change_generation: self
                    .state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0),
                controller_at_event: self
                    .state
                    .objects
                    .get(&object_id)
                    .map(|object| object.controller)
                    .ok_or(EngineError::Illegal("attacking token missing"))?,
            };
            parsed.push((
                object_id,
                CombatAttackAssignment {
                    attacker,
                    defender,
                    defending_player: option.defending_player_id,
                },
            ));
        }
        let combat = self
            .state
            .combat
            .as_mut()
            .filter(|combat| combat.attackers_declared)
            .ok_or(EngineError::Illegal(
                "no declared-attacker combat for attacking tokens",
            ))?;
        for (object_id, assignment) in &parsed {
            combat.attacking.push(*object_id);
            combat.attack_assignments.insert(*object_id, *assignment);
        }
        Ok(parsed
            .into_iter()
            .map(|(object_id, assignment)| {
                self.wire_attack_assignment(
                    object_id,
                    assignment.defender,
                    assignment.defending_player,
                )
            })
            .collect())
    }

    /// CR 508.1b candidates. The same generation-bound edges are published to the client and
    /// accepted by declaration, so neither Battle protection nor planeswalker control is inferred
    /// outside the engine.
    pub(super) fn legal_attack_assignments(&self, player: PlayerId) -> Vec<rv1::AttackAssignment> {
        let defenders = self.attack_defenders();
        self.eligible_attacker_ids(player)
            .into_iter()
            .flat_map(|attacker| {
                defenders.iter().map(move |(defender, defending_player)| {
                    self.wire_attack_assignment(attacker, *defender, *defending_player)
                })
            })
            .collect()
    }

    fn parse_attack_assignment(
        &self,
        assignment: &rv1::AttackAssignment,
        active_player: PlayerId,
    ) -> Result<CombatAttackAssignment, EngineError> {
        let candidates: Vec<_> = self
            .legal_attack_assignments(active_player)
            .into_iter()
            .filter(|candidate| candidate.attacker_object_id == assignment.attacker_object_id)
            .collect();
        let legal = if assignment.defender.is_none()
            && assignment.attacker_zone_change_generation == 0
            && assignment.defender_zone_change_generation == 0
            && assignment.defending_player_id == 0
            && candidates.len() == 1
        {
            candidates[0]
        } else {
            candidates
                .into_iter()
                .find(|candidate| candidate == assignment)
                .ok_or(EngineError::Illegal(
                    "illegal or stale attack defender assignment",
                ))?
        };
        let attacker = self
            .trigger_object_ref(legal.attacker_object_id)
            .ok_or(EngineError::Illegal("stale attacker"))?;
        let defender_ref = legal
            .defender
            .as_ref()
            .ok_or(EngineError::Illegal("missing attack defender"))?;
        let defender = match rv1::TargetRefKind::try_from(defender_ref.kind) {
            Ok(rv1::TargetRefKind::Player) => {
                CombatDefenderTarget::Player(defender_ref.object_id as PlayerId)
            }
            Ok(rv1::TargetRefKind::Permanent) => {
                CombatDefenderTarget::Permanent(TriggerObjectRef {
                    object_id: defender_ref.object_id,
                    zone_change_generation: legal.defender_zone_change_generation,
                    controller_at_event: self
                        .controller_of(defender_ref.object_id)
                        .ok_or(EngineError::Illegal("attack defender disappeared"))?,
                })
            }
            _ => return Err(EngineError::Illegal("invalid attack defender kind")),
        };
        Ok(CombatAttackAssignment {
            attacker,
            defender,
            defending_player: legal.defending_player_id,
        })
    }

    /// Snapshot derived characteristics once per declaration query. All consumers share this
    /// relation and the complete-declaration evaluator; no client reconstructs restrictions.
    fn block_graph(&self, defending_player: PlayerId) -> BlockGraph {
        let mut attacker_ids = self
            .state
            .combat
            .as_ref()
            .map(|combat| {
                combat
                    .attacking
                    .iter()
                    .copied()
                    .filter(|oid| {
                        combat
                            .attack_assignments
                            .get(oid)
                            .map(|assignment| assignment.defending_player)
                            .or_else(|| self.state.sole_defending_player_id())
                            == Some(defending_player)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        attacker_ids.sort_unstable();
        attacker_ids.dedup();
        let mut blocker_ids = self
            .state
            .player_idx(defending_player)
            .map(|idx| {
                self.state.players[idx]
                    .battlefield
                    .iter()
                    .copied()
                    .filter(|oid| self.base_blocker_eligible(*oid, defending_player))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        blocker_ids.sort_unstable();
        blocker_ids.dedup();
        let values = |ids: Vec<ObjectId>| {
            ids.into_iter()
                .filter_map(|oid| {
                    let characteristics = self.characteristics(oid)?;
                    let restrictions = self.combat_restrictions_for(oid, &characteristics);
                    Some((oid, characteristics, restrictions))
                })
                .collect::<Vec<_>>()
        };
        let attackers = values(attacker_ids);
        let blockers = values(blocker_ids);
        BlockGraph {
            attackers: attackers.iter().map(|(oid, _, _)| *oid).collect(),
            blockers: blockers.iter().map(|(oid, _, _)| *oid).collect(),
            edges: blockers
                .iter()
                .map(|(bid, b, br)| {
                    attackers
                        .iter()
                        .enumerate()
                        .filter_map(|(a, (aid, c, ar))| {
                            self.can_block((*aid, c, ar), (*bid, b, br), defending_player)
                                .then_some(a)
                        })
                        .collect()
                })
                .collect(),
            minimum: attackers
                .iter()
                .map(|(_, c, r)| {
                    r.minimum_blockers
                        .unwrap_or(1)
                        .max(if c.has_keyword(Keyword::Menace) { 2 } else { 1 })
                        as usize
                })
                .collect(),
            maximum: attackers
                .iter()
                .map(|(_, _, r)| {
                    if r.cant_be_blocked {
                        Some(0)
                    } else {
                        r.maximum_blockers.map(|max| max as usize)
                    }
                })
                .collect(),
            must_block: blockers
                .iter()
                .map(|(oid, _, _)| self.state.objects[oid].must_block_if_able)
                .collect(),
        }
    }

    pub(super) fn blocking_options(
        &self,
        defending_player: PlayerId,
    ) -> (Vec<rv1::BlockPair>, Vec<ObjectId>) {
        let analysis = self.block_graph(defending_player).analyze();
        (analysis.pairs, analysis.required)
    }

    /// CR 509.1b pair restrictions use current characteristics, never targetability. Counts and
    /// requirements are checked on the complete declaration by BlockGraph.
    fn can_block(
        &self,
        attacker: (ObjectId, &Characteristics, &CombatRestriction),
        blocker: (ObjectId, &Characteristics, &CombatRestriction),
        defending_player: PlayerId,
    ) -> bool {
        let (attacker_id, attacker, ar) = attacker;
        let (blocker_id, blocker, br) = blocker;
        if br.cant_block || ar.cant_be_blocked {
            return false;
        }
        if ar.cant_be_blocked_by.iter().any(|filter| {
            super::characteristics::permanent_matches_filter_characteristics(
                &self.state,
                filter,
                blocker_id,
                blocker,
            )
        }) || br.cant_block_creatures_matching.iter().any(|filter| {
            super::characteristics::permanent_matches_filter_characteristics(
                &self.state,
                filter,
                attacker_id,
                attacker,
            )
        }) {
            return false;
        }
        if attacker.has_keyword(Keyword::Flying)
            && !blocker.has_keyword(Keyword::Flying)
            && !blocker.has_keyword(Keyword::Reach)
        {
            return false;
        }
        if attacker.has_keyword(Keyword::Intimidate)
            && !blocker.is_artifact()
            && !attacker
                .colors
                .iter()
                .any(|color| blocker.colors.contains(color))
        {
            return false;
        }
        if attacker
            .protections
            .iter()
            .any(|quality| quality.matches(&blocker.colors, &blocker.types))
        {
            return false;
        }
        for evasion in &attacker.evasions {
            let tricerules_cards::Evasion::Landwalk { land_subtype } = evasion;
            if self.state.player_idx(defending_player).is_some_and(|idx| {
                self.state.players[idx].battlefield.iter().any(|oid| {
                    self.characteristics(*oid).is_some_and(|land| {
                        land.controller == defending_player
                            && land.has_type("Land")
                            && land.has_type(land_subtype)
                    })
                })
            }) {
                return false;
            }
        }
        true
    }

    pub(super) fn active_player_has_eligible_attackers(&self) -> bool {
        let ap = self.state.active_player_id();
        !self.eligible_attacker_ids(ap).is_empty()
    }

    pub(super) fn defending_player_has_eligible_blockers(&self) -> bool {
        self.state
            .sole_defending_player_id()
            .is_some_and(|defender| !self.blocking_options(defender).0.is_empty())
    }

    /// CR 508.1d: the active player's creatures that MUST be declared as attackers this combat —
    /// untapped, not summoning-sick (unless Haste), non-Defender creatures with `must_attack_if_able`,
    /// when a defending player exists to attack. Single source of truth shared by `set_attackers`
    /// enforcement and the client-facing `LegalActions` gate (Juggernaut, Goblin Brigand, Crazed Goblin).
    pub(super) fn required_attacker_ids(&self) -> Vec<ObjectId> {
        // "when a defending player exists to attack" — the count does not matter here, only that
        // there is someone (CR 508.1a).
        if self.state.defending_player_ids().is_empty() {
            return Vec::new();
        }
        let ap = self.state.active_player_id();
        let mut out = Vec::new();
        let Some(ap_idx) = self.state.player_idx(ap) else {
            return out;
        };
        for &oid in &self.state.players[ap_idx].battlefield {
            let Some(obj) = self.state.objects.get(&oid) else {
                continue;
            };
            if !obj.must_attack_if_able {
                continue;
            }
            if self.attacker_illegality(oid, ap).is_some() {
                continue;
            }
            out.push(oid);
        }
        out
    }

    pub(super) fn set_attackers(
        &mut self,
        assignments: &[rv1::AttackAssignment],
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
            if !assignments
                .iter()
                .any(|assignment| assignment.attacker_object_id == oid)
            {
                return Err(EngineError::Illegal(
                    "must-attack creature not declared as attacker",
                ));
            }
        }

        if assignments.is_empty() {
            self.clear_step_mana_pools();
            self.state.combat = None;
            self.state.turn_step = TurnStep::EndCombat;
            if let Some(i) = self.state.player_idx(ap) {
                self.state.priority_idx = i;
            }
            self.state.passes_since_stack_change = 0;
            let mut b2 = RuledEventBatch::default();
            b2.events
                .push(ev_log("No attackers — skipped to end combat".to_string()));
            b2.events.push(ev_phase(self, rv1::PhaseId::EndCombat));
            b2.events.push(ev_priority_changed(self));
            fill_legal(&mut b2, self);
            return Ok(b2);
        }
        let mut list = Vec::new();
        let mut parsed_assignments = HashMap::new();
        let mut seen_attackers = HashSet::new();
        for assignment in assignments {
            let oid = assignment.attacker_object_id;
            if !seen_attackers.insert(oid) {
                return Err(EngineError::Illegal("duplicate attacker"));
            }
            if let Some(reason) = self.attacker_illegality(oid, ap) {
                return Err(EngineError::Illegal(reason));
            }
            let parsed = self.parse_attack_assignment(assignment, ap)?;
            parsed_assignments.insert(oid, parsed);
            list.push(oid);
        }
        let mut tapping_attackers = Vec::new();
        for &oid in &list {
            // CR 702.20b — Vigilance: attacking doesn't cause this creature to tap.
            let has_vigilance =
                self.effective_has_keyword(oid, tricerules_cards::Keyword::Vigilance);
            if !has_vigilance {
                tapping_attackers.push(oid);
            }
        }
        let mut tap_events = self.tap_permanents(ap, &tapping_attackers);
        let attackers_for_event = list.clone();
        if let Some(c) = self.state.combat.as_mut() {
            c.attacking = list;
            c.attack_assignments = parsed_assignments;
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
                attack_assignments: parsed_assignments,
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
        self.clear_step_mana_pools();
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
                    assignments: assignments.to_vec(),
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
        let attacks = attackers_for_event
            .into_iter()
            .filter_map(|attacker_id| {
                let assignment = self
                    .state
                    .combat
                    .as_ref()?
                    .attack_assignments
                    .get(&attacker_id)?;
                Some(AttackEdgeSnapshot {
                    attacker: assignment.attacker,
                    defender: assignment.defender,
                    defending_player: assignment.defending_player,
                })
            })
            .collect();
        tap_events.push(GameEvent::AttackersDeclared {
            attacking_player: ap,
            attacks,
        });
        self.fire_triggers(&tap_events);
        b.events.push(ev_priority_changed(self));
        Ok(b)
    }

    pub(super) fn set_blockers(
        &mut self,
        pairs: &[rv1::BlockPair],
    ) -> Result<RuledEventBatch, EngineError> {
        let defending_player = self
            .state
            .sole_defending_player_id()
            .ok_or(EngineError::Illegal("defender missing"))?;
        let graph = self.block_graph(defending_player);
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
            // CR 509.1a: likewise for blocking — control, not ownership.
            if self.controller_of(p.blocker_id) != Some(defending_player) {
                return Err(EngineError::Illegal("not your blocker"));
            }
            if !self
                .characteristics(p.blocker_id)
                .is_some_and(|value| value.is_creature())
            {
                return Err(EngineError::Illegal("blocker not creature"));
            }
            if bobj.tapped {
                return Err(EngineError::Illegal("blocker tapped"));
            }
            // Evasion check: flying (CR 702.9b), intimidate (CR 702.13b), etc.
            let legal_pair = graph
                .blockers
                .iter()
                .position(|oid| *oid == p.blocker_id)
                .zip(graph.attackers.iter().position(|oid| *oid == p.attacker_id))
                .is_some_and(|(b, a)| graph.edges[b].contains(&a));
            if !legal_pair {
                return Err(EngineError::Illegal(
                    "blocker cannot block this attacker (evasion)",
                ));
            }
            attacker_to_blockers
                .entry(p.attacker_id)
                .or_default()
                .push(p.blocker_id);
        }
        // Validate restrictions before requirements, without mutating combat or replay state.
        for (a, attacker) in graph.attackers.iter().enumerate() {
            let count = attacker_to_blockers.get(attacker).map_or(0, Vec::len);
            if !graph.count_is_legal(a, count) {
                return Err(EngineError::Illegal("Illegal blocks."));
            }
        }
        let satisfied = graph
            .blockers
            .iter()
            .zip(&graph.must_block)
            .filter(|(oid, must)| **must && seen_blockers.contains(oid))
            .count();
        if satisfied != graph.maximum_requirements() {
            return Err(EngineError::Illegal(
                "block declaration must satisfy the maximum possible blocking requirements",
            ));
        }
        let block_edges: Vec<BlockEdgeSnapshot> = pairs
            .iter()
            .map(|pair| {
                Ok(BlockEdgeSnapshot {
                    attacker: self
                        .trigger_object_ref(pair.attacker_id)
                        .ok_or(EngineError::Illegal("attacker characteristics missing"))?,
                    blocker: self
                        .trigger_object_ref(pair.blocker_id)
                        .ok_or(EngineError::Illegal("blocker characteristics missing"))?,
                })
            })
            .collect::<Result<_, EngineError>>()?;
        // CR 702.19: trample attackers with 1+ blockers also require explicit damage assignment
        // (to split damage between blockers and the defending player).
        let damage_assignment_needed = attacker_to_blockers.iter().any(|(atk_id, blks)| {
            self.attacker_needs_explicit_damage_assignment(*atk_id, blks.len())
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
        self.clear_step_mana_pools();
        // MTG timing: blockers are declared in declare-blockers, then players get priority
        // before the game advances into combat-damage where damage is actually dealt.
        self.state.turn_step = TurnStep::DeclareBlockers;
        if let Some(i) = self.state.player_idx(self.state.active_player_id()) {
            self.state.priority_idx = i;
        }
        self.state.passes_since_stack_change = 0;
        b.events
            .push(ev_log(format!("P{} {}", defending_player, block_line)));
        self.fire_triggers(&[GameEvent::BlockersDeclared { edges: block_edges }]);
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
                if self.attacker_needs_explicit_damage_assignment(*atk_id, blks.len()) {
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
            self.clear_step_mana_pools();
            self.state.turn_step = TurnStep::FirstStrikeDamage;
            if let Some(i) = self.state.player_idx(ap) {
                self.state.priority_idx = i;
            }
            self.state.passes_since_stack_change = 0;
            events.push(ev_log("First strike combat damage dealt.".to_string()));
            events.push(ev_phase(self, rv1::PhaseId::FirstStrikeDamage));
            self.resolve_combat_damage(&c2, DamagePass::FirstStrike, events)?;
            if matches!(
                self.state.pending_replacement_event,
                Some(super::replacement::PendingReplacementEvent::Damage(_))
            ) {
                return Ok(());
            }
            // CR 510.2 + 704: SBAs run between damage steps so creatures with lethal damage are
            // moved to graveyards before the regular step decides who deals damage.
            self.apply_sbas(events)?;
            events.push(ev_priority_changed(self));
        } else {
            // Emit PhaseChanged before resolving damage so the C++ client clears its
            // stack-object set before any combat damage triggers are pushed (StackPushed).
            self.state.combat = None;
            self.clear_step_mana_pools();
            self.state.turn_step = TurnStep::CombatDamage;
            if let Some(i) = self.state.player_idx(ap) {
                self.state.priority_idx = i;
            }
            self.state.passes_since_stack_change = 0;
            events.push(ev_log("Combat damage dealt.".to_string()));
            events.push(ev_phase(self, rv1::PhaseId::CombatDamage));
            self.resolve_combat_damage(&c_init, DamagePass::Normal, events)?;
            if matches!(
                self.state.pending_replacement_event,
                Some(super::replacement::PendingReplacementEvent::Damage(_))
            ) {
                return Ok(());
            }
            self.apply_sbas(events)?;
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
        if self.try_park_ordered_combat_damage(c, pass, events)? {
            return Ok(());
        }
        let ap = self.state.active_player_id();
        let mut life_lost: BTreeMap<PlayerId, i32> = BTreeMap::new();
        // (controller_id, amount) pairs — collected during damage assignment, applied after.
        let mut lifelink_gains: Vec<(PlayerId, u32)> = Vec::new();
        // Finalized source-recipient occurrences, collected for damage triggers after the
        // simultaneous combat batch has been applied.
        let mut damage_dealt_events: Vec<DamageEvent> = Vec::new();

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
            // CR 702.15b: lifelink credits the source's *controller*, not its owner.
            let att_controller = self
                .state
                .objects
                .get(&att)
                .map(|o| o.controller)
                .unwrap_or(ap);
            let att_has_trample = self.effective_has_keyword(att, Keyword::Trample);
            let defending_player = c
                .attack_assignments
                .get(&att)
                .map(|assignment| assignment.defending_player)
                .unwrap_or(ap);

            let blockers = c.blockers.get(&att).map(|v| v.as_slice()).unwrap_or(&[]);

            if blockers.is_empty() {
                // Unblocked: deal full power to defending player — only if the attacker assigns
                // damage this pass (CR 510.4).
                if attacker_participates {
                    let Some(recipient) = self.combat_defender_recipient(c, att) else {
                        continue;
                    };
                    let damage_event = DamageEvent::combat(
                        att,
                        att_controller,
                        object_display_name(&self.state, self.registry, att),
                        recipient,
                        att_power,
                    );
                    let Some(result) = self.process_or_park_combat_damage(
                        damage_event.clone(),
                        att_has_deathtouch,
                        att_has_lifelink,
                        events,
                    ) else {
                        return Ok(());
                    };
                    let p = match recipient {
                        DamageRecipient::Player(player) => {
                            if let Some(index) = self.state.player_idx(player) {
                                super::history::commit_life_change(
                                    &mut self.state,
                                    index,
                                    -(result.dealt as i32),
                                );
                                *life_lost.entry(player).or_default() += result.dealt as i32;
                                result.dealt
                            } else {
                                0
                            }
                        }
                        DamageRecipient::Permanent(_) => self.commit_damage_result(
                            &damage_event,
                            result,
                            att_has_deathtouch,
                            events,
                        ),
                    };
                    if p > 0 {
                        let mut dealt_event = damage_event;
                        dealt_event.amount = p;
                        damage_dealt_events.push(dealt_event);
                    }
                    // CR 702.15b: attacker with lifelink causes its controller to gain that much life.
                    if att_has_lifelink && p > 0 {
                        lifelink_gains.push((att_controller, p));
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
                let blk_controller = self
                    .state
                    .objects
                    .get(&blk)
                    .map(|o| o.controller)
                    .unwrap_or(defending_player);
                if blocker_participates {
                    let Some(result) = self.process_or_park_combat_damage(
                        DamageEvent::combat(
                            blk,
                            blk_controller,
                            object_display_name(&self.state, self.registry, blk),
                            DamageRecipient::Permanent(att),
                            bpw,
                        ),
                        blk_has_deathtouch,
                        blk_has_lifelink,
                        events,
                    ) else {
                        return Ok(());
                    };
                    let dmg_to_att = result.dealt;
                    if dmg_to_att > 0 {
                        damage_dealt_events.push(DamageEvent::combat(
                            blk,
                            blk_controller,
                            object_display_name(&self.state, self.registry, blk),
                            DamageRecipient::Permanent(att),
                            dmg_to_att,
                        ));
                    }
                    if let Some(af) = self.state.objects.get_mut(&att) {
                        af.damage += dmg_to_att;
                        // CR 702.2b / CR 704.5h: any damage from a deathtouch source is lethal.
                        if blk_has_deathtouch && dmg_to_att > 0 {
                            af.deathtouch_damage = true;
                        }
                    }
                    // CR 702.15b: blocker with lifelink gains life = damage dealt to attacker.
                    if blk_has_lifelink && dmg_to_att > 0 {
                        lifelink_gains.push((blk_controller, dmg_to_att));
                    }
                }
                if attacker_participates {
                    let Some(result) = self.process_or_park_combat_damage(
                        DamageEvent::combat(
                            att,
                            att_controller,
                            object_display_name(&self.state, self.registry, att),
                            DamageRecipient::Permanent(blk),
                            att_power,
                        ),
                        att_has_deathtouch,
                        att_has_lifelink,
                        events,
                    ) else {
                        return Ok(());
                    };
                    let dmg_to_blk = result.dealt;
                    if dmg_to_blk > 0 {
                        damage_dealt_events.push(DamageEvent::combat(
                            att,
                            att_controller,
                            object_display_name(&self.state, self.registry, att),
                            DamageRecipient::Permanent(blk),
                            dmg_to_blk,
                        ));
                    }
                    if let Some(bf) = self.state.objects.get_mut(&blk) {
                        bf.damage += dmg_to_blk;
                        // CR 702.2b: any damage from attacker with deathtouch is lethal.
                        if att_has_deathtouch && dmg_to_blk > 0 {
                            bf.deathtouch_damage = true;
                        }
                    }
                    // CR 702.15b: attacker with lifelink gains life = damage dealt to blocker.
                    if att_has_lifelink && dmg_to_blk > 0 {
                        lifelink_gains.push((att_controller, dmg_to_blk));
                    }
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
                        let controller = self
                            .state
                            .objects
                            .get(&blk)
                            .map(|o| o.controller)
                            .unwrap_or(defending_player);
                        let participates = object_participates_in_pass(self, c, pass, blk, false)
                            && self.state.objects.get(&blk).map(|o| o.zone)
                                == Some(Zone::Battlefield);
                        (blk, pw, has_ll, has_dt, controller, participates)
                    })
                    .collect();
                // Damage is dealt simultaneously, but prevention and lifelink are applied per
                // source.  Tracking each blocker separately is important when a prevention shield
                // prevents only part of the combined damage (CR 615.1, 702.15b).
                let mut total_blocker_damage = 0;
                let mut any_blocker_deathtouch_hit = false;
                let mut blocker_damage_dealt = Vec::new();
                for (
                    blocker_id,
                    blocker_power,
                    has_lifelink,
                    has_deathtouch,
                    blocker_controller,
                    participates,
                ) in &blocker_info
                {
                    if !*participates || *blocker_power == 0 {
                        continue;
                    }
                    let Some(result) = self.process_or_park_combat_damage(
                        DamageEvent::combat(
                            *blocker_id,
                            *blocker_controller,
                            object_display_name(&self.state, self.registry, *blocker_id),
                            DamageRecipient::Permanent(att),
                            *blocker_power,
                        ),
                        *has_deathtouch,
                        *has_lifelink,
                        events,
                    ) else {
                        return Ok(());
                    };
                    let dealt = result.dealt;
                    total_blocker_damage += dealt;
                    if dealt > 0 {
                        damage_dealt_events.push(DamageEvent::combat(
                            *blocker_id,
                            *blocker_controller,
                            object_display_name(&self.state, self.registry, *blocker_id),
                            DamageRecipient::Permanent(att),
                            dealt,
                        ));
                    }
                    if *has_deathtouch && dealt > 0 {
                        any_blocker_deathtouch_hit = true;
                    }
                    blocker_damage_dealt.push((*has_lifelink, *blocker_controller, dealt));
                }
                if let Some(af) = self.state.objects.get_mut(&att) {
                    af.damage += total_blocker_damage;
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
                    let mut total_att_lifelink: u32 = 0;
                    for &(blk, dmg) in pairs {
                        let Some(result) = self.process_or_park_combat_damage(
                            DamageEvent::combat(
                                att,
                                att_controller,
                                object_display_name(&self.state, self.registry, att),
                                DamageRecipient::Permanent(blk),
                                dmg,
                            ),
                            att_has_deathtouch,
                            att_has_lifelink,
                            events,
                        ) else {
                            return Ok(());
                        };
                        let dmg_to_blk = result.dealt;
                        if dmg_to_blk > 0 {
                            damage_dealt_events.push(DamageEvent::combat(
                                att,
                                att_controller,
                                object_display_name(&self.state, self.registry, att),
                                DamageRecipient::Permanent(blk),
                                dmg_to_blk,
                            ));
                        }
                        if let Some(bf) = self.state.objects.get_mut(&blk) {
                            bf.damage += dmg_to_blk;
                            // CR 702.2b: any damage from attacker with deathtouch is lethal.
                            if att_has_deathtouch && dmg_to_blk > 0 {
                                bf.deathtouch_damage = true;
                            }
                        }
                        total_att_lifelink += dmg_to_blk;
                    }
                    // CR 702.19: deal trample excess damage to the attacked recipient.
                    let player_trample_dmg =
                        c.trample_player_damage.get(&att).copied().unwrap_or(0);
                    if player_trample_dmg > 0 {
                        if let Some(recipient) = self.combat_defender_recipient(c, att) {
                            let damage_event = DamageEvent::combat(
                                att,
                                att_controller,
                                object_display_name(&self.state, self.registry, att),
                                recipient,
                                player_trample_dmg,
                            );
                            let Some(result) = self.process_or_park_combat_damage(
                                damage_event.clone(),
                                att_has_deathtouch,
                                att_has_lifelink,
                                events,
                            ) else {
                                return Ok(());
                            };
                            let trample_after = match recipient {
                                DamageRecipient::Player(player) => {
                                    if let Some(index) = self.state.player_idx(player) {
                                        super::history::commit_life_change(
                                            &mut self.state,
                                            index,
                                            -(result.dealt as i32),
                                        );
                                        *life_lost.entry(player).or_default() +=
                                            result.dealt as i32;
                                        result.dealt
                                    } else {
                                        0
                                    }
                                }
                                DamageRecipient::Permanent(_) => self.commit_damage_result(
                                    &damage_event,
                                    result,
                                    att_has_deathtouch,
                                    events,
                                ),
                            };
                            if trample_after > 0 {
                                let mut dealt_event = damage_event;
                                dealt_event.amount = trample_after;
                                damage_dealt_events.push(dealt_event);
                            }
                            total_att_lifelink += trample_after;
                        }
                    }
                    // CR 702.15b: attacker with lifelink gains life = damage dealt to all blockers.
                    if att_has_lifelink && total_att_lifelink > 0 {
                        lifelink_gains.push((att_controller, total_att_lifelink));
                    }
                }
                // CR 702.15b: each participating blocker with lifelink gains life equal to the
                // damage it actually dealt, after prevention.
                for (has_lifelink, blocker_controller, dealt) in blocker_damage_dealt {
                    if has_lifelink && dealt > 0 {
                        lifelink_gains.push((blocker_controller, dealt));
                    }
                }
            }
        }
        for (player, lost) in life_lost {
            if lost == 0 {
                continue;
            }
            if let Some(index) = self.state.player_idx(player) {
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                        player_id: player,
                        new_total: self.state.players[index].life,
                        delta: -lost,
                    })),
                });
            }
        }
        // Apply lifelink gains. Each entry is one creature's gain and stays a separate life-gain
        // event (CR 702.15b): two lifelink creatures dealing damage in this step trigger a
        // "whenever you gain life" ability twice, so these are deliberately not summed per player.
        let mut trigger_events = Vec::new();
        for (pid, amount) in lifelink_gains {
            if let Some(event) = super::resolution::life::apply_life_gain_without_triggers(
                self, events, pid, amount, "lifelink",
            ) {
                trigger_events.push(event);
            }
        }
        trigger_events.extend(
            damage_dealt_events
                .into_iter()
                .map(|event| GameEvent::DamageDealt { event }),
        );
        self.fire_triggers(&trigger_events);
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

/// CR 508.1k: true if `oid` is currently an attacker in the active combat.
pub(super) fn is_attacking(state: &GameState, oid: ObjectId) -> bool {
    state
        .combat
        .as_ref()
        .is_some_and(|combat| combat.attacking.contains(&oid))
}

/// CR 509.1g: true if `oid` is currently a blocker in the active combat.
pub(super) fn is_blocking(state: &GameState, oid: ObjectId) -> bool {
    state.combat.as_ref().is_some_and(|combat| {
        combat
            .blockers
            .values()
            .any(|blockers| blockers.contains(&oid))
    })
}

/// CR 508/509: true if `oid` is currently an attacker or a blocker in the active combat.
pub(super) fn is_attacking_or_blocking(state: &GameState, oid: ObjectId) -> bool {
    is_attacking(state, oid) || is_blocking(state, oid)
}
