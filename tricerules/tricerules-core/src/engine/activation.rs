//! CR 602.3: engine-owned multi-actor target announcement before ability payment.
use super::combat::priority_locked_for_combat_declaration;
use super::payment::PreparedPaymentCosts;
use super::targeting::{compute_ability_targets, TargetSourceIdentity};
use super::*;

#[derive(Clone)]
pub(super) struct PendingAbilityActivationInternal {
    effective: EffectiveActivatedAbility,
    prepared_costs: Option<PreparedPaymentCosts>,
    announcement: super::casting::AnnouncedAbilityActivation,
}

fn target_ref(target: &rv1::AbilityActivationTarget) -> rv1::TargetRef {
    rv1::TargetRef {
        object_id: target.object_id,
        group_index: target.group_index,
        kind: rv1::TargetRefKind::Permanent as i32,
        ..Default::default()
    }
}

impl GameEngine {
    fn eligible_activation_opponents(
        &self,
        pending: &rv1::PendingAbilityActivation,
        ability: &ActivatedAbilityDef,
    ) -> Vec<PlayerId> {
        let groups = self.activation_target_groups(pending, ability);
        let Some(group) = groups.groups.get(1) else {
            return Vec::new();
        };
        self.state
            .players
            .iter()
            .filter(|candidate| {
                !candidate.has_lost
                    && self
                        .state
                        .are_opponents(pending.actor_player_id, candidate.id)
                    && group.valid_permanent_ids.iter().any(|oid| {
                        self.characteristics(*oid)
                            .is_some_and(|c| c.controller == candidate.id)
                    })
            })
            .map(|candidate| candidate.id)
            .collect()
    }

    /// Reconcile accepted independent mana work and departures without undoing that work.
    pub(super) fn reconcile_pending_ability_activation(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let Some(mut pending) = self.state.pending_ability_activation.clone() else {
            return;
        };
        let source_current = self
            .state
            .player_idx(pending.actor_player_id)
            .is_some_and(|idx| {
                !self.state.players[idx].has_lost
                    && self.state.players[idx]
                        .battlefield
                        .contains(&pending.source_object_id)
                    && self
                        .state
                        .objects
                        .get(&pending.source_object_id)
                        .is_some_and(|source| {
                            source.zone == Zone::Battlefield
                                && source.controller == pending.actor_player_id
                                && self.payment_object_ref(source.id).zone_change_generation
                                    == pending.source_zone_change_generation
                        })
            });
        let ability = self
            .pending_ability_activation_internal
            .as_ref()
            .map(|internal| &internal.effective.definition);
        let valid = source_current
            && ability.is_some_and(|ability| {
                pending.stage() == rv1::AbilityActivationStage::Payment
                    || pending.announced_targets.first().is_some_and(|own| {
                        self.activation_target_receipt(own.object_id, 0) == *own
                            && self
                                .activation_target_groups(&pending, ability)
                                .groups
                                .first()
                                .is_some_and(|group| {
                                    group.valid_permanent_ids.contains(&own.object_id)
                                })
                    })
            });
        if !valid {
            self.state.pending_ability_activation = None;
            self.pending_ability_activation_internal = None;
            events.push(events::ev_log("Unfinished ability activation abandoned because its actor, source or required announcement is unavailable.".into()));
            return;
        }
        // After the answer, preserve fixed target/player receipts even if a target departed.
        if pending.stage() == rv1::AbilityActivationStage::Payment {
            return;
        }
        let ability = ability.unwrap();
        let eligible = self.eligible_activation_opponents(&pending, ability);
        if eligible.is_empty() {
            self.state.pending_ability_activation = None;
            self.pending_ability_activation_internal = None;
            events.push(events::ev_log("Unfinished ability activation abandoned because no opponent can complete its target choice.".into()));
            return;
        }
        let old = pending.clone();
        pending.valid_opponent_ids = eligible;
        if let Some(selected) = pending
            .chosen_opponent_id
            .filter(|selected| pending.valid_opponent_ids.contains(selected))
        {
            // A refreshed candidate list retires stale generation/revision receipts.
            self.set_activation_opponent(&mut pending, ability, selected)
                .expect("eligible opponent has a legal target");
        } else {
            pending.stage = rv1::AbilityActivationStage::ChooseOpponent as i32;
            pending.deciding_player_id = pending.actor_player_id;
            pending.chosen_opponent_id = None;
            pending.target_group = None;
            pending.target_candidates.clear();
        }
        if pending != old {
            pending.revision = pending.revision.saturating_add(1);
            self.state.pending_ability_activation = Some(pending);
        }
    }
    fn activation_target_receipt(
        &self,
        object_id: ObjectId,
        group_index: u32,
    ) -> rv1::AbilityActivationTarget {
        rv1::AbilityActivationTarget {
            object_id,
            zone_change_generation: self.payment_object_ref(object_id).zone_change_generation,
            group_index,
        }
    }

    fn activation_target_groups(
        &self,
        pending: &rv1::PendingAbilityActivation,
        ability: &ActivatedAbilityDef,
    ) -> rv1::SpellTargets {
        compute_ability_targets(
            self,
            pending.actor_player_id,
            TargetSourceIdentity::captured(
                pending.source_object_id,
                pending.source_zone_change_generation,
            ),
            &ability.effect,
            ability.targeting.as_ref(),
        )
    }

    fn set_activation_opponent(
        &self,
        pending: &mut rv1::PendingAbilityActivation,
        ability: &ActivatedAbilityDef,
        opponent: PlayerId,
    ) -> Result<(), EngineError> {
        if !pending.valid_opponent_ids.contains(&opponent) {
            return Err(EngineError::Illegal("ineligible opponent for activation"));
        }
        let mut group = self
            .activation_target_groups(pending, ability)
            .groups
            .into_iter()
            .nth(1)
            .ok_or(EngineError::Illegal("missing opponent target group"))?;
        group.valid_permanent_ids.retain(|oid| {
            self.characteristics(*oid)
                .is_some_and(|c| c.controller == opponent)
        });
        if group.valid_permanent_ids.is_empty() {
            return Err(EngineError::Illegal(
                "opponent has no legal creature choice",
            ));
        }
        pending.target_candidates = group
            .valid_permanent_ids
            .iter()
            .map(|&oid| self.activation_target_receipt(oid, 1))
            .collect();
        pending.target_group = Some(group);
        pending.chosen_opponent_id = Some(opponent);
        pending.deciding_player_id = opponent;
        pending.stage = rv1::AbilityActivationStage::OpponentTarget as i32;
        pending.valid_opponent_ids.clear();
        Ok(())
    }

    pub(super) fn begin_ability_activation(
        &mut self,
        player: PlayerId,
        command: &rv1::BeginAbilityActivation,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.priority_player_id() != player
            || self.state.turn_step == TurnStep::Cleanup
            || self.state.blocking_choice().is_some()
            || self.state.pending_spell_cast.is_some()
            || priority_locked_for_combat_declaration(&self.state)
        {
            return Err(EngineError::Illegal(
                "ability announcement is not legal now",
            ));
        }
        let source = self
            .state
            .objects
            .get(&command.source_object_id)
            .filter(|object| object.zone == Zone::Battlefield && object.controller == player)
            .ok_or(EngineError::Illegal(
                "activation source is not controlled on the battlefield",
            ))?;
        if self.payment_object_ref(source.id).zone_change_generation
            != command.expected_zone_change_generation
        {
            return Err(EngineError::Illegal("stale activation source generation"));
        }
        let effective = self
            .effective_activated_abilities(source.id)
            .into_iter()
            .find(|effective| effective.slot == command.ability_index)
            .ok_or(EngineError::Illegal("missing activated ability"))?;
        if !effective.definition.requires_opponent_target_choice()
            || !self.ability_activatable(
                source.id,
                command.ability_index as usize,
                &effective.definition,
            )
        {
            return Err(EngineError::Illegal(
                "ability does not support this opponent-choice announcement",
            ));
        }
        let own = command
            .own_target
            .as_ref()
            .ok_or(EngineError::Illegal("missing controller target"))?;
        let mut pending = rv1::PendingAbilityActivation {
            transaction_id: self.state.next_ability_activation_transaction_id,
            revision: 1,
            actor_player_id: player,
            deciding_player_id: player,
            source_object_id: source.id,
            source_zone_change_generation: command.expected_zone_change_generation,
            ability_index: command.ability_index,
            reserved_object_id: self.state.next_object_id,
            announced_targets: vec![*own],
            source_description: events::object_display_name(&self.state, self.registry, source.id),
            ..Default::default()
        };
        let groups = self.activation_target_groups(&pending, &effective.definition);
        if own.group_index != 0
            || self.activation_target_receipt(own.object_id, 0) != *own
            || !groups
                .groups
                .first()
                .is_some_and(|group| group.valid_permanent_ids.contains(&own.object_id))
        {
            return Err(EngineError::Illegal(
                "illegal or stale controller creature target",
            ));
        }
        pending.valid_opponent_ids =
            self.eligible_activation_opponents(&pending, &effective.definition);
        if pending.valid_opponent_ids.is_empty() {
            return Err(EngineError::Illegal(
                "no opponent can complete this activation",
            ));
        }
        let selected = command.opponent_player_id.or_else(|| {
            (pending.valid_opponent_ids.len() == 1).then(|| pending.valid_opponent_ids[0])
        });
        if let Some(opponent) = selected {
            self.set_activation_opponent(&mut pending, &effective.definition, opponent)?;
        }
        let announcement = self.capture_ability_announcement(
            player,
            source.id,
            &effective,
            &[target_ref(own)],
            0,
            self.effective_card_identity(source.id)
                .map(|(id, face)| (id.to_string(), face))
                .ok_or(EngineError::Illegal("bad activation face"))?,
        )?;
        self.state.next_ability_activation_transaction_id =
            pending.transaction_id.saturating_add(1);
        self.state.pending_ability_activation = Some(pending);
        self.pending_ability_activation_internal = Some(PendingAbilityActivationInternal {
            effective,
            prepared_costs: None,
            announcement,
        });
        Ok(RuledEventBatch::default())
    }

    pub(super) fn submit_ability_activation_choice(
        &mut self,
        player: PlayerId,
        command: &rv1::SubmitAbilityActivationChoice,
    ) -> Result<RuledEventBatch, EngineError> {
        let mut pending = self
            .state
            .pending_ability_activation
            .clone()
            .ok_or(EngineError::Illegal("no pending ability activation"))?;
        if pending.deciding_player_id != player
            || pending.transaction_id != command.transaction_id
            || pending.revision != command.expected_revision
        {
            return Err(EngineError::Illegal(
                "wrong actor or stale activation choice",
            ));
        }
        let internal = self
            .pending_ability_activation_internal
            .as_ref()
            .ok_or(EngineError::Illegal("missing activation snapshot"))?;
        let ability = &internal.effective.definition;
        let mut prepared = None;
        let mut announcement = internal.announcement.clone();
        match pending.stage() {
            rv1::AbilityActivationStage::ChooseOpponent => {
                if command.target.is_some() {
                    return Err(EngineError::Illegal(
                        "opponent selection cannot replace announced targets",
                    ));
                }
                self.set_activation_opponent(
                    &mut pending,
                    ability,
                    command
                        .opponent_player_id
                        .ok_or(EngineError::Illegal("missing opponent choice"))?,
                )?;
            }
            rv1::AbilityActivationStage::OpponentTarget => {
                if command.opponent_player_id.is_some() {
                    return Err(EngineError::Illegal(
                        "target reply cannot change the chosen opponent",
                    ));
                }
                let target = command
                    .target
                    .as_ref()
                    .ok_or(EngineError::Illegal("missing opponent target"))?;
                if target.group_index != 1
                    || !pending.target_candidates.contains(target)
                    || self.activation_target_receipt(target.object_id, 1) != *target
                    || self
                        .characteristics(target.object_id)
                        .is_none_or(|c| Some(c.controller) != pending.chosen_opponent_id)
                {
                    return Err(EngineError::Illegal("illegal or stale opponent target"));
                }
                pending.announced_targets.push(*target);
                let targets: Vec<_> = pending.announced_targets.iter().map(target_ref).collect();
                super::targeting::validate_ability_targets(
                    self,
                    pending.actor_player_id,
                    TargetSourceIdentity::captured(
                        pending.source_object_id,
                        pending.source_zone_change_generation,
                    ),
                    &ability.effect,
                    ability.targeting.as_ref(),
                    &targets,
                )?;
                if pending.announced_targets.iter().any(|target| {
                    self.activation_target_receipt(target.object_id, target.group_index) != *target
                }) {
                    return Err(EngineError::Illegal("announced target incarnation changed"));
                }
                if !self.ability_activatable(
                    pending.source_object_id,
                    pending.ability_index as usize,
                    ability,
                ) || self
                    .payment_object_ref(pending.source_object_id)
                    .zone_change_generation
                    != pending.source_zone_change_generation
                {
                    return Err(EngineError::Illegal(
                        "announced activation is no longer legal",
                    ));
                }
                let costs = self.prepare_ability_costs(
                    pending.actor_player_id,
                    self.state
                        .player_idx(pending.actor_player_id)
                        .ok_or(EngineError::UnknownPlayer(pending.actor_player_id))?,
                    pending.source_object_id,
                    &ability.costs,
                    &[],
                    &[],
                    &[],
                    0,
                    self.targeting_cost_increase(
                        pending.actor_player_id,
                        TargetingCostAction::ActivatedAbilities,
                        &targets,
                    ),
                    self.activated_mana_reduction(
                        pending.actor_player_id,
                        pending.source_object_id,
                        ability,
                    )?,
                )?;
                pending.locked_total_cost = costs.total_cost_label()?;
                self.append_ability_announcement_targets(
                    &mut announcement,
                    &[target_ref(target)],
                    pending.chosen_opponent_id,
                );
                prepared = Some(costs);
                pending.stage = rv1::AbilityActivationStage::Payment as i32;
                pending.deciding_player_id = pending.actor_player_id;
                pending.target_group = None;
                pending.target_candidates.clear();
            }
            rv1::AbilityActivationStage::Payment => {
                return Err(EngineError::Illegal(
                    "activation targets are already locked",
                ))
            }
        }
        pending.revision = pending.revision.saturating_add(1);
        if let Some(costs) = prepared {
            self.pending_ability_activation_internal
                .as_mut()
                .unwrap()
                .prepared_costs = Some(costs);
        }
        self.pending_ability_activation_internal
            .as_mut()
            .unwrap()
            .announcement = announcement;
        self.state.pending_ability_activation = Some(pending);
        Ok(RuledEventBatch::default())
    }

    pub(super) fn paying_ability_activation(&self, player: PlayerId) -> bool {
        self.state
            .pending_ability_activation
            .as_ref()
            .is_some_and(|pending| {
                pending.actor_player_id == player
                    && pending.stage() == rv1::AbilityActivationStage::Payment
                    && self.state.pending_resolution.is_none()
            })
    }

    pub(super) fn prepare_pending_ability_payment(
        &self,
        player: PlayerId,
        command: &rv1::CommitAbilityActivation,
    ) -> Result<PreparedPaymentCosts, EngineError> {
        let pending = self
            .state
            .pending_ability_activation
            .as_ref()
            .ok_or(EngineError::Illegal("no pending ability activation"))?;
        if !self.paying_ability_activation(player)
            || pending.transaction_id != command.transaction_id
            || pending.revision != command.expected_revision
        {
            return Err(EngineError::Illegal(
                "wrong actor or stale activation payment",
            ));
        }
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let source = self
            .state
            .objects
            .get(&pending.source_object_id)
            .filter(|source| {
                source.zone == Zone::Battlefield
                    && source.controller == player
                    && self.state.players[idx].battlefield.contains(&source.id)
                    && !self.state.players[idx].has_lost
            })
            .ok_or(EngineError::Illegal(
                "activation source cannot pay its tap cost",
            ))?;
        if self.payment_object_ref(source.id).zone_change_generation
            != pending.source_zone_change_generation
        {
            return Err(EngineError::Illegal(
                "activation source incarnation changed",
            ));
        }
        self.check_tappable(source.id, &source.card_id)?;
        let mut costs = self
            .pending_ability_activation_internal
            .as_ref()
            .and_then(|internal| internal.prepared_costs.as_ref())
            .cloned()
            .ok_or(EngineError::Illegal("missing locked activation cost"))?;
        costs.eligible_restricted_mana = self.eligible_restricted_mana_for_ability(idx, source.id);
        costs.restricted_mana = command.restricted_mana.clone();
        costs.flex_payments = command.flex_payments.clone();
        Ok(costs)
    }

    pub(super) fn commit_ability_activation(
        &mut self,
        player: PlayerId,
        command: &rv1::CommitAbilityActivation,
    ) -> Result<RuledEventBatch, EngineError> {
        let costs = self.prepare_pending_ability_payment(player, command)?;
        let source = self
            .state
            .pending_ability_activation
            .as_ref()
            .unwrap()
            .source_object_id;
        let plan = self.finish_ability_payment(player, source, costs, command.payment.as_ref())?;
        let receipt = self.commit_cost_transaction(plan)?;
        let internal = self.pending_ability_activation_internal.take().unwrap();
        self.state.pending_ability_activation = None;
        Ok(self.finish_announced_ability(internal.announcement, receipt))
    }

    pub(super) fn cancel_ability_activation(
        &mut self,
        player: PlayerId,
        command: &rv1::CancelAbilityActivation,
    ) -> Result<RuledEventBatch, EngineError> {
        let pending = self
            .state
            .pending_ability_activation
            .as_ref()
            .ok_or(EngineError::Illegal("no pending ability activation"))?;
        if pending.actor_player_id != player
            || pending.transaction_id != command.transaction_id
            || pending.revision != command.expected_revision
        {
            return Err(EngineError::Illegal(
                "wrong actor or stale activation cancellation",
            ));
        }
        self.state.pending_ability_activation = None;
        self.pending_ability_activation_internal = None;
        Ok(RuledEventBatch::default())
    }

    pub(super) fn pending_ability_activation_for(
        &self,
        player: PlayerId,
    ) -> Option<rv1::PendingAbilityActivation> {
        self.state
            .pending_ability_activation
            .clone()
            .map(|mut pending| {
                if pending.deciding_player_id != player || self.state.pending_resolution.is_some() {
                    pending.valid_opponent_ids.clear();
                    pending.target_group = None;
                    pending.target_candidates.clear();
                    pending.locked_total_cost.clear();
                    pending.payment_preview = None;
                    pending.eligible_restricted_mana_group_ids.clear();
                } else if pending.stage() == rv1::AbilityActivationStage::Payment {
                    let command = rv1::CommitAbilityActivation {
                        transaction_id: pending.transaction_id,
                        expected_revision: pending.revision,
                        ..Default::default()
                    };
                    pending.eligible_restricted_mana_group_ids = self
                        .prepare_pending_ability_payment(player, &command)
                        .map(|costs| costs.eligible_restricted_mana)
                        .unwrap_or_default();
                    let mut preview = self.preview_payment(
                        player,
                        &rv1::PreviewPayment {
                            transaction_id: pending.transaction_id,
                            revision: self.state.command_index,
                            commit_ability_activation: Some(command),
                            ..Default::default()
                        },
                    );
                    if let Some(selection) = preview.selection.as_mut() {
                        selection.expected_state_revision =
                            self.state.command_index.saturating_add(1);
                    }
                    pending.payment_preview = Some(preview);
                }
                pending
            })
    }
}
