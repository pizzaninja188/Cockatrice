//! CR 602.3 target announcements, driven by the authored group order rather than card names.
use super::super::targeting::{
    ability_announcement_target_is_legal, validate_ability_announcement_group,
};
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetProgress {
    Ready,
    Impossible,
}

pub(super) fn canonical_target(target: &rv1::AbilityActivationTarget) -> rv1::TargetRef {
    rv1::TargetRef {
        object_id: target.object_id,
        group_index: target.group_index,
        kind: target.kind,
        expected_zone_change_generation: (target.kind == rv1::TargetRefKind::Permanent as i32)
            .then_some(target.zone_change_generation),
        ..Default::default()
    }
}

fn normalize_reply(
    mut target: rv1::AbilityActivationTarget,
    legacy_singleton: bool,
) -> Result<rv1::AbilityActivationTarget, EngineError> {
    // The old singleton messages exclusively described permanents. New lists never infer a
    // domain from a numeric ID, including when that ID names both a player and a permanent.
    if legacy_singleton && target.kind == rv1::TargetRefKind::Unspecified as i32 {
        target.kind = rv1::TargetRefKind::Permanent as i32;
    }
    match target.kind() {
        rv1::TargetRefKind::Permanent => Ok(target),
        rv1::TargetRefKind::Player if target.zone_change_generation == 0 => Ok(target),
        _ => Err(EngineError::Illegal(
            "invalid staged target domain or generation",
        )),
    }
}

fn same_target(left: &rv1::AbilityActivationTarget, right: &rv1::AbilityActivationTarget) -> bool {
    left.kind == right.kind
        && left.object_id == right.object_id
        && left.zone_change_generation == right.zone_change_generation
}

impl GameEngine {
    fn announcement_context<'a>(
        &'a self,
        internal: &'a PendingAbilityActivationInternal,
    ) -> TargetResolutionContext<'a> {
        let ability = &internal.effective.definition;
        TargetResolutionContext {
            effects: &ability.effect,
            targeting: ability.targeting.as_ref(),
            trigger_context: TriggerContext::default(),
            program: self.program_for_activation_occurrence(&internal.effective.occurrence),
        }
    }

    fn announcement_source(
        &self,
        pending: &rv1::PendingAbilityActivation,
        internal: &PendingAbilityActivationInternal,
    ) -> TargetSourceIdentity {
        TargetSourceIdentity::captured_for_ability(
            pending.source_object_id,
            pending.source_zone_change_generation,
            &internal.effective.definition,
        )
    }

    fn announcement_groups(
        &self,
        pending: &rv1::PendingAbilityActivation,
        internal: &PendingAbilityActivationInternal,
    ) -> rv1::SpellTargets {
        let context = self.announcement_context(internal);
        compute_ability_targets_with_program(
            self,
            pending.actor_player_id,
            self.announcement_source(pending, internal),
            context.effects,
            context.targeting,
            context.program,
        )
    }

    fn chooser_controls_group(internal: &PendingAbilityActivationInternal, index: u32) -> bool {
        internal
            .effective
            .definition
            .targeting
            .as_ref()
            .and_then(|targeting| targeting.groups.get(index as usize))
            .is_some_and(|group| group.controlled_by_chooser)
    }

    fn announcement_candidates(
        &self,
        pending: &rv1::PendingAbilityActivation,
        internal: &PendingAbilityActivationInternal,
        group_index: u32,
        chooser: PlayerId,
    ) -> Vec<rv1::AbilityActivationTarget> {
        let context = self.announcement_context(internal);
        let source = self.announcement_source(pending, internal);
        let Some(targeting) = &internal.effective.definition.targeting else {
            return Vec::new();
        };
        let Some(group) = targeting.groups.get(group_index as usize) else {
            return Vec::new();
        };
        let permitted = |target: &rv1::AbilityActivationTarget| {
            (!group.controlled_by_chooser
                || (target.kind == rv1::TargetRefKind::Permanent as i32
                    && self
                        .characteristics(target.object_id)
                        .is_some_and(|c| c.controller == chooser)))
                && !pending.announced_targets.iter().any(|prior| {
                    let distinct = group.distinct_from.contains(&prior.group_index)
                        || targeting
                            .groups
                            .get(prior.group_index as usize)
                            .is_some_and(|earlier| earlier.distinct_from.contains(&group_index));
                    distinct && same_target(prior, target)
                })
                && ability_announcement_target_is_legal(
                    self,
                    pending.actor_player_id,
                    source,
                    &canonical_target(target),
                    context,
                )
        };
        let mut candidates = Vec::new();
        for player in &self.state.players {
            if player.has_lost {
                continue;
            }
            let target = rv1::AbilityActivationTarget {
                object_id: player.id as ObjectId,
                zone_change_generation: 0,
                group_index,
                kind: rv1::TargetRefKind::Player as i32,
            };
            if permitted(&target) {
                candidates.push(target);
            }
            for &oid in &player.battlefield {
                let target = rv1::AbilityActivationTarget {
                    object_id: oid,
                    zone_change_generation: self.payment_object_ref(oid).zone_change_generation,
                    group_index,
                    kind: rv1::TargetRefKind::Permanent as i32,
                };
                if permitted(&target) {
                    candidates.push(target);
                }
            }
        }
        candidates
    }

    fn announcement_choosers(
        &self,
        pending: &rv1::PendingAbilityActivation,
        internal: &PendingAbilityActivationInternal,
    ) -> Vec<PlayerId> {
        let groups = self.announcement_groups(pending, internal);
        let any_opponents = self.state.players.iter().any(|player| {
            !player.has_lost && self.state.are_opponents(pending.actor_player_id, player.id)
        });
        self.state
            .players
            .iter()
            .filter(|player| {
                !player.has_lost
                    && (self.state.are_opponents(pending.actor_player_id, player.id)
                        || (internal.replacing_departed_chooser && !any_opponents))
                    && groups
                        .groups
                        .iter()
                        .skip(pending.active_target_group_index as usize)
                        .filter(|group| group.chosen_by_opponent)
                        .all(|group| {
                            self.announcement_candidates(
                                pending,
                                internal,
                                group.group_index,
                                player.id,
                            )
                            .len()
                                >= group.min as usize
                        })
            })
            .map(|player| player.id)
            .collect()
    }

    fn advance_target_announcement(
        &self,
        pending: &mut rv1::PendingAbilityActivation,
        internal: &mut PendingAbilityActivationInternal,
        auto_choose_initial_opponent: bool,
    ) -> Result<TargetProgress, EngineError> {
        let groups = self.announcement_groups(pending, internal);
        pending.target_group_count = groups.groups.len() as u32;
        let index = pending.active_target_group_index as usize;
        if index == groups.groups.len() {
            let idx = self
                .state
                .player_idx(pending.actor_player_id)
                .ok_or(EngineError::UnknownPlayer(pending.actor_player_id))?;
            let ability = &internal.effective.definition;
            if !self.ability_activatable(
                pending.source_object_id,
                pending.ability_index as usize,
                ability,
            ) || !self.ability_announcement_controllers_current(&internal.announcement)
            {
                return Ok(TargetProgress::Impossible);
            }
            let targets = pending
                .announced_targets
                .iter()
                .map(canonical_target)
                .collect::<Vec<_>>();
            super::super::targeting::validate_ability_targets_with_context_and_program(
                self,
                pending.actor_player_id,
                self.announcement_source(pending, internal),
                &targets,
                self.announcement_context(internal),
            )?;
            let costs = self.prepare_ability_costs(
                pending.actor_player_id,
                idx,
                pending.source_object_id,
                &ability.costs,
                &pending.flex_payments,
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
            internal.prepared_costs = Some(costs);
            pending.stage = rv1::AbilityActivationStage::Payment as i32;
            pending.deciding_player_id = pending.actor_player_id;
            pending.target_group = None;
            pending.target_candidates.clear();
            pending.valid_opponent_ids.clear();
            return Ok(TargetProgress::Ready);
        }
        let mut group = groups
            .groups
            .get(index)
            .cloned()
            .ok_or(EngineError::Illegal("unknown staged target group"))?;
        let chooser = if group.chosen_by_opponent {
            if pending.chosen_opponent_id.is_some_and(|player| {
                self.state
                    .player_idx(player)
                    .is_none_or(|idx| self.state.players[idx].has_lost)
            }) {
                internal.replacing_departed_chooser = true;
                pending.chosen_opponent_id = None;
            }
            let eligible = self.announcement_choosers(pending, internal);
            if eligible.is_empty() {
                return Ok(TargetProgress::Impossible);
            }
            if pending.chosen_opponent_id.is_none()
                && auto_choose_initial_opponent
                && !internal.replacing_departed_chooser
                && eligible.len() == 1
            {
                pending.chosen_opponent_id = Some(eligible[0]);
            }
            let Some(selected) = pending.chosen_opponent_id else {
                pending.valid_opponent_ids = eligible;
                pending.stage = rv1::AbilityActivationStage::ChooseOpponent as i32;
                pending.deciding_player_id = pending.actor_player_id;
                pending.target_group = None;
                pending.target_candidates.clear();
                return Ok(TargetProgress::Ready);
            };
            if !eligible.contains(&selected) {
                return Ok(TargetProgress::Impossible);
            }
            internal.handed_to_opponent = true;
            selected
        } else {
            pending.actor_player_id
        };
        let candidates =
            self.announcement_candidates(pending, internal, group.group_index, chooser);
        if candidates.len() < group.min as usize {
            return Ok(TargetProgress::Impossible);
        }
        // Rebuild the projection from typed candidates. In particular, a player/object numeric
        // collision cannot put the object into this offer based on player legality.
        group.valid_permanent_ids = candidates
            .iter()
            .filter(|c| c.kind == rv1::TargetRefKind::Permanent as i32)
            .map(|c| c.object_id)
            .collect();
        group.valid_permanent_targets = candidates
            .iter()
            .filter(|c| c.kind == rv1::TargetRefKind::Permanent as i32)
            .map(|c| rv1::LegalPermanentTargetCandidate {
                object_id: c.object_id,
                zone_change_generation: c.zone_change_generation,
            })
            .collect();
        group.valid_stack_ids.clear();
        group.valid_graveyard_ids.clear();
        group.can_target_self = candidates.iter().any(|c| {
            c.kind == rv1::TargetRefKind::Player as i32
                && c.object_id == pending.actor_player_id as u32
        });
        group.can_target_opponent = candidates.iter().any(|c| {
            c.kind == rv1::TargetRefKind::Player as i32
                && self
                    .state
                    .are_opponents(pending.actor_player_id, c.object_id as PlayerId)
        });
        pending.stage = rv1::AbilityActivationStage::TargetSelection as i32;
        pending.deciding_player_id = chooser;
        pending.valid_opponent_ids.clear();
        pending.target_group = Some(group);
        pending.target_candidates = candidates;
        Ok(TargetProgress::Ready)
    }

    fn accept_announcement_group(
        &self,
        pending: &mut rv1::PendingAbilityActivation,
        internal: &mut PendingAbilityActivationInternal,
        targets: Vec<rv1::AbilityActivationTarget>,
    ) -> Result<(), EngineError> {
        let group_index = pending.active_target_group_index;
        let offered = self.announcement_candidates(
            pending,
            internal,
            group_index,
            pending.deciding_player_id,
        );
        if targets
            .iter()
            .any(|target| target.group_index != group_index || !offered.contains(target))
        {
            return Err(EngineError::Illegal("illegal or stale announced target"));
        }
        let mut all = pending
            .announced_targets
            .iter()
            .map(canonical_target)
            .collect::<Vec<_>>();
        all.extend(targets.iter().map(canonical_target));
        validate_ability_announcement_group(
            self,
            pending.actor_player_id,
            self.announcement_source(pending, internal),
            &all,
            self.announcement_context(internal),
            group_index as usize,
        )?;
        let required_controller = Self::chooser_controls_group(internal, group_index)
            .then_some(pending.deciding_player_id);
        let targets_as_refs = targets.iter().map(canonical_target).collect::<Vec<_>>();
        self.append_ability_announcement_targets(
            &mut internal.announcement,
            &targets_as_refs,
            required_controller,
        );
        pending.announced_targets.extend(targets);
        pending.active_target_group_index += 1;
        Ok(())
    }

    pub(super) fn begin_targeted_ability_activation(
        &mut self,
        player: PlayerId,
        idx: usize,
        source: &GameObject,
        effective: EffectiveActivatedAbility,
        command: &rv1::BeginAbilityActivation,
    ) -> Result<RuledEventBatch, EngineError> {
        let ability = &effective.definition;
        if !ability.requires_opponent_target_choice()
            || !self.ability_activatable(source.id, command.ability_index as usize, ability)
        {
            return Err(EngineError::Illegal(
                "ability does not support this opponent-choice announcement",
            ));
        }
        let costs = self.prepare_ability_costs(
            player,
            idx,
            source.id,
            &ability.costs,
            &command.flex_payments,
            &[],
            &[],
            0,
            0,
            self.activated_mana_reduction(player, source.id, ability)?,
        )?;
        super::super::payment::demand::validated_phyrexian_choices(
            &costs.mana,
            &command.flex_payments,
        )?;
        if command.opponent_player_id.is_some_and(|opponent| {
            !self.state.are_opponents(player, opponent)
                || self
                    .state
                    .player_idx(opponent)
                    .is_none_or(|idx| self.state.players[idx].has_lost)
        }) {
            return Err(EngineError::Illegal("invalid activation opponent"));
        }
        let announcement = self.capture_ability_announcement(
            player,
            source.id,
            &effective,
            &[],
            0,
            self.effective_card_identity(source.id)
                .map(|(id, face)| (id.to_string(), face))
                .ok_or(EngineError::Illegal("bad activation face"))?,
        )?;
        let mut pending = rv1::PendingAbilityActivation {
            transaction_id: self.state.next_ability_activation_transaction_id,
            revision: 1,
            actor_player_id: player,
            deciding_player_id: player,
            source_object_id: source.id,
            source_zone_change_generation: command.expected_zone_change_generation,
            ability_index: command.ability_index,
            reserved_object_id: self.state.next_object_id,
            source_description: events::object_display_name(&self.state, self.registry, source.id),
            chosen_opponent_id: command.opponent_player_id,
            flex_payments: command.flex_payments.clone(),
            ..Default::default()
        };
        let mut internal = PendingAbilityActivationInternal {
            effective,
            route: PendingAbilityActivationRoute::Targets,
            prepared_costs: None,
            announcement,
            handed_to_opponent: false,
            replacing_departed_chooser: false,
        };
        if self.advance_target_announcement(&mut pending, &mut internal, true)?
            == TargetProgress::Impossible
        {
            return Err(EngineError::Illegal(
                "no player can complete this activation's targets",
            ));
        }
        if let Some(legacy) = &command.own_target {
            let group = pending
                .target_group
                .as_ref()
                .ok_or(EngineError::Illegal("unexpected legacy controller target"))?;
            if pending.deciding_player_id != player
                || group.chosen_by_opponent
                || group.min != 1
                || group.max != 1
            {
                return Err(EngineError::Illegal(
                    "legacy Begin requires a singleton controller group",
                ));
            }
            self.accept_announcement_group(
                &mut pending,
                &mut internal,
                vec![normalize_reply(*legacy, true)?],
            )?;
            if self.advance_target_announcement(&mut pending, &mut internal, true)?
                == TargetProgress::Impossible
            {
                return Err(EngineError::Illegal(
                    "no player can complete this activation's targets",
                ));
            }
        }
        self.state.next_ability_activation_transaction_id =
            pending.transaction_id.saturating_add(1);
        self.state.pending_ability_activation = Some(pending);
        self.pending_ability_activation_internal = Some(internal);
        Ok(RuledEventBatch::default())
    }

    pub(in crate::engine) fn submit_ability_activation_choice(
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
        let mut internal = self
            .pending_ability_activation_internal
            .clone()
            .ok_or(EngineError::Illegal("missing activation snapshot"))?;
        match pending.stage() {
            rv1::AbilityActivationStage::ChooseOpponent => {
                if command.target.is_some() || !command.targets.is_empty() {
                    return Err(EngineError::Illegal(
                        "opponent choice cannot also select targets",
                    ));
                }
                let opponent = command
                    .opponent_player_id
                    .ok_or(EngineError::Illegal("missing chosen opponent"))?;
                if !pending.valid_opponent_ids.contains(&opponent)
                    || !self
                        .announcement_choosers(&pending, &internal)
                        .contains(&opponent)
                {
                    return Err(EngineError::Illegal("ineligible activation opponent"));
                }
                pending.chosen_opponent_id = Some(opponent);
            }
            rv1::AbilityActivationStage::TargetSelection => {
                if command.opponent_player_id.is_some()
                    || (command.target.is_some() && !command.targets.is_empty())
                {
                    return Err(EngineError::Illegal(
                        "target reply cannot change the chooser or mix target formats",
                    ));
                }
                let targets = if let Some(target) = command.target {
                    let group = pending
                        .target_group
                        .as_ref()
                        .ok_or(EngineError::Illegal("missing target group"))?;
                    if group.min != 1 || group.max != 1 {
                        return Err(EngineError::Illegal(
                            "legacy reply requires a singleton group",
                        ));
                    }
                    vec![normalize_reply(target, true)?]
                } else {
                    command
                        .targets
                        .iter()
                        .copied()
                        .map(|target| normalize_reply(target, false))
                        .collect::<Result<Vec<_>, _>>()?
                };
                self.accept_announcement_group(&mut pending, &mut internal, targets)?;
            }
            rv1::AbilityActivationStage::Payment => {
                return Err(EngineError::Illegal(
                    "activation targets are already locked",
                ))
            }
        }
        if self.advance_target_announcement(&mut pending, &mut internal, true)?
            == TargetProgress::Impossible
        {
            self.state.pending_ability_activation = None;
            self.pending_ability_activation_internal = None;
            return Ok(RuledEventBatch { events: vec![events::ev_log("Unfinished ability activation abandoned because its remaining targets cannot be chosen.".into())], ..Default::default() });
        }
        pending.revision = pending.revision.saturating_add(1);
        self.state.pending_ability_activation = Some(pending);
        self.pending_ability_activation_internal = Some(internal);
        Ok(RuledEventBatch::default())
    }

    pub(super) fn announcement_receipts_are_current(
        &self,
        pending: &rv1::PendingAbilityActivation,
    ) -> bool {
        let Some(internal) = &self.pending_ability_activation_internal else {
            return false;
        };
        self.ability_announcement_controllers_current(&internal.announcement)
            && pending.announced_targets.iter().all(|target| {
                ability_announcement_target_is_legal(
                    self,
                    pending.actor_player_id,
                    self.announcement_source(pending, internal),
                    &canonical_target(target),
                    self.announcement_context(internal),
                )
            })
    }

    pub(super) fn refresh_target_announcement(
        &mut self,
        pending: &mut rv1::PendingAbilityActivation,
    ) -> bool {
        let Some(mut internal) = self.pending_ability_activation_internal.clone() else {
            return false;
        };
        let old = pending.clone();
        if !matches!(
            self.advance_target_announcement(pending, &mut internal, false),
            Ok(TargetProgress::Ready)
        ) {
            return false;
        }
        if *pending != old {
            pending.revision = pending.revision.saturating_add(1);
        }
        self.pending_ability_activation_internal = Some(internal);
        self.state.pending_ability_activation = Some(pending.clone());
        true
    }
}
