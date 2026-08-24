use super::combat::priority_locked_for_combat_declaration;
use super::events::{ev_log, ev_priority_changed, format_spell_targets_log};
use super::legal_actions::fill_legal;
#[cfg(test)]
use super::payment::{commit_mana_payment, pay_mana, plan_mana_payment_with_reduction};
use super::payment::{PaidCardCost, SacrificeSnapshot};
use super::targeting::{
    capture_stack_target, validate_ability_targets, validate_spell_targets, TargetSourceIdentity,
};
use super::*;

fn format_paid_card_costs_log(costs: &[PaidCardCost]) -> String {
    let phrases: Vec<_> = costs.iter().map(PaidCardCost::log_phrase).collect();
    match phrases.as_slice() {
        [] => String::new(),
        [only] => format!(" {only}"),
        [first, second] => format!(" {first} and {second}"),
        _ => format!(
            " {}, and {}",
            phrases[..phrases.len() - 1].join(", "),
            phrases.last().expect("nonempty cost phrases")
        ),
    }
}

/// CR 702.8b: true if the card face is castable at instant speed (is an instant, or has flash).
pub(super) fn castable_at_instant_speed(face: &tricerules_cards::FaceRef<'_>) -> bool {
    face.is_instant || face.keywords.contains(&tricerules_cards::Keyword::Flash)
}

fn command_satisfies_cast_cost_condition(
    selections: &[rv1::CastCostGroupSelection],
    condition: tricerules_cards::CastCostReceiptCondition,
) -> bool {
    selections.iter().any(|selection| {
        selection.group_index == condition.group_index
            && selection.option_index == condition.option_index
    }) == condition.expected_selected
}

impl GameEngine {
    pub(super) fn cast_spell(
        &mut self,
        player: PlayerId,
        command: &rv1::CastSpell,
    ) -> Result<RuledEventBatch, EngineError> {
        let targets = command.targets.as_slice();
        let x_value = command.x_value;
        let flex_payments = command.flex_payments.as_slice();
        let face_index = command.face_index as usize;
        let selected_modes = command.selected_modes.as_slice();
        let cost_selections = command.cost_selections.as_slice();
        let cast_cost_group_selections = command.cast_cost_group_selections.as_slice();
        let restricted_mana = command.restricted_mana.as_slice();
        let cast_method = match rv1::CastMethod::try_from(command.cast_method) {
            Ok(rv1::CastMethod::Normal) => SpellCastMethod::Normal,
            Ok(rv1::CastMethod::Flashback) => SpellCastMethod::Flashback,
            Ok(rv1::CastMethod::Harmonize) => SpellCastMethod::Harmonize,
            Ok(rv1::CastMethod::Unspecified) | Err(_) => {
                return Err(EngineError::Illegal("missing or invalid cast method"));
            }
        };
        if self.state.turn_step == TurnStep::Cleanup {
            return Err(EngineError::Illegal("no spells during cleanup"));
        }
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let source = command
            .source
            .as_ref()
            .and_then(|source| source.location.as_ref())
            .ok_or(EngineError::Illegal("missing cast source"))?;
        let from_hand = matches!(source, rv1::cast_source::Location::HandIndex(_));
        let (oid, exile_permission_scope) = match source {
            rv1::cast_source::Location::HandIndex(hand_index) => {
                if cast_method != SpellCastMethod::Normal {
                    return Err(EngineError::Illegal(
                        "hand casts require the normal cast method",
                    ));
                }
                (
                    *self.state.players[idx]
                        .hand
                        .get(*hand_index as usize)
                        .ok_or(EngineError::Illegal("bad hand index"))?,
                    None,
                )
            }
            rv1::cast_source::Location::GraveyardObjectId(source_oid) => {
                if !self.state.players[idx].graveyard.contains(source_oid) {
                    return Err(EngineError::Illegal("card is not in your graveyard"));
                }
                if cast_method == SpellCastMethod::Normal {
                    return Err(EngineError::Illegal(
                        "graveyard casts require an alternative cast method",
                    ));
                }
                (*source_oid, None)
            }
            rv1::cast_source::Location::ExileObjectId(source_oid) => {
                if cast_method != SpellCastMethod::Normal {
                    return Err(EngineError::Illegal(
                        "exile casts require the normal cast method",
                    ));
                }
                let object = self
                    .state
                    .objects
                    .get(source_oid)
                    .ok_or(EngineError::Illegal("unknown exile object"))?;
                let generation = self
                    .state
                    .zone_change_generation
                    .get(source_oid)
                    .copied()
                    .unwrap_or(0);
                let permission = self
                    .state
                    .active_exile_play_permissions
                    .iter()
                    .find(|permission| {
                        permission.object_id == *source_oid
                            && permission.player_id == player
                            && permission.zone_change_generation == generation
                            && match permission.scope {
                                ExilePlayPermissionScope::CastFace(expected) => {
                                    expected == face_index
                                }
                                ExilePlayPermissionScope::PlayCard => true,
                            }
                    })
                    .ok_or(EngineError::Illegal(
                        "card has no cast-from-exile permission",
                    ))?;
                if object.zone != Zone::Exile {
                    return Err(EngineError::Illegal("illegal cast from exile"));
                }
                (*source_oid, Some(permission.scope))
            }
        };
        let has_multiple_cast_options =
            super::legal_actions::cast_option_count_for_source(self, player, source) > 1;
        let target_source = TargetSourceIdentity::spell_face(self, oid, face_index);
        let card_id = self.state.objects.get(&oid).unwrap().card_id.clone();
        let def = self
            .registry
            .get(&card_id)
            .ok_or_else(|| EngineError::MissingCard(card_id.clone()))?;
        let face = def
            .face(face_index)
            .ok_or(EngineError::Illegal("bad face index"))?;
        if from_hand && !def.face_available_from_hand(face_index) {
            return Err(EngineError::Illegal("face cannot be cast from hand"));
        }
        if matches!(
            exile_permission_scope,
            Some(ExilePlayPermissionScope::CastFace(_))
        ) && !face.is_permanent()
        {
            return Err(EngineError::Illegal(
                "Adventure permission requires a permanent face",
            ));
        }
        if face.is_land {
            return Err(EngineError::Illegal("use play land"));
        }
        let face_is_sorcery = face.is_sorcery;
        let face_instant_speed = castable_at_instant_speed(&face);
        let face_mana = match cast_method {
            SpellCastMethod::Normal => face.mana_cost.clone(),
            SpellCastMethod::Flashback => face
                .flashback_cost
                .clone()
                .ok_or(EngineError::Illegal("card has no flashback cost"))?,
            SpellCastMethod::Harmonize => face
                .harmonize_cost
                .clone()
                .ok_or(EngineError::Illegal("card has no harmonize cost"))?,
        };
        let face_name = face.name.to_string();
        let face_effects: Vec<SpellEffectKind> = face.spell_effect.to_vec();
        let face_targeting = face.targeting.clone();
        let modal_spell = face.modal_spell.clone();
        let additional_costs = face.additional_costs.clone();
        let cast_cost_groups = face.cast_cost_groups.clone();
        let cost_modifiers = face.cost_modifiers.clone();
        let eligible_restricted_mana = self.eligible_restricted_mana_for_spell(idx, face);
        let sorcery_ok = super::priority::sorcery_speed_available(&self.state, player);
        let instant_ok = super::priority::instant_timing_step_allowed(&self.state);
        let conditional_instant = face.instant_speed_cast_cost.is_some_and(|condition| {
            command_satisfies_cast_cost_condition(cast_cost_group_selections, condition)
        });
        if face_is_sorcery {
            if !(sorcery_ok || instant_ok && conditional_instant) {
                return Err(EngineError::Illegal("sorcery speed only"));
            }
        } else if face_instant_speed {
            if !instant_ok {
                return Err(EngineError::Illegal("instant timing"));
            }
        } else if !(sorcery_ok || instant_ok && conditional_instant) {
            return Err(EngineError::Illegal("sorcery speed only"));
        }
        if priority_locked_for_combat_declaration(&self.state) {
            return Err(EngineError::Illegal(
                "cannot cast until attack or block declaration is complete",
            ));
        }
        // As in `pass_priority`: `dispatch_command`'s blocking gate normally catches these first;
        // this is the local refusal with a message that names casting.
        match self.state.blocking_choice() {
            Some(BlockingChoice::TriggerTarget) => {
                return Err(EngineError::Illegal(
                    "must choose trigger target before casting",
                ));
            }
            Some(BlockingChoice::TriggerOrder) => {
                return Err(EngineError::Illegal(
                    "must order simultaneous triggers before casting",
                ));
            }
            Some(BlockingChoice::Resolution) => {
                return Err(EngineError::Illegal(
                    "must submit resolution choice before casting",
                ));
            }
            None => {}
        }
        let has_x = face_mana.has_x();
        if x_value != 0 && !has_x {
            return Err(EngineError::Illegal("x_value given but cost has no {X}"));
        }
        let chosen_x = if has_x { x_value } else { 0 };

        let mut public_targets: Vec<rv1::TargetRef> = Vec::new();
        let mut chosen_modes: Vec<ChosenMode> = Vec::new();
        let mut chosen_mode_indices: Vec<u32> = Vec::new();
        let mut chosen_mode_labels: Vec<String> = Vec::new();
        let mut extra_generic = 0;
        if let Some(modal) = &modal_spell {
            if !targets.is_empty() {
                return Err(EngineError::Illegal(
                    "modal spells use selected_modes targets",
                ));
            }
            if selected_modes.len() < modal.min_modes as usize
                || selected_modes.len() > modal.max_modes as usize
            {
                return Err(EngineError::Illegal("illegal number of selected modes"));
            }
            let mut seen = HashSet::new();
            let mut ordered: Vec<&rv1::SelectedSpellMode> = selected_modes.iter().collect();
            ordered.sort_by_key(|selection| selection.mode_index);
            for selection in ordered {
                if !seen.insert(selection.mode_index) {
                    return Err(EngineError::Illegal("a mode may be selected only once"));
                }
                let mode = modal
                    .modes
                    .get(selection.mode_index as usize)
                    .ok_or(EngineError::Illegal("bad spell mode index"))?;
                validate_spell_targets(
                    self,
                    player,
                    target_source,
                    &mode.effects,
                    mode.targeting.as_ref(),
                    &selection.targets,
                )?;
                for effect in &mode.effects {
                    if let SpellEffectKind::DamageTargets {
                        amount,
                        division,
                        extra_mana_per_target,
                        ..
                    } = effect
                    {
                        if matches!(division, DamageDivision::ChooseAtCast) {
                            let allocated: u32 = selection
                                .targets
                                .iter()
                                .map(|target| target.damage_amount)
                                .sum();
                            if allocated
                                != amount.resolve_unconditional(chosen_x).ok_or(
                                    EngineError::Illegal(
                                        "DamageTargets cannot use a conditional amount",
                                    ),
                                )?
                                || selection
                                    .targets
                                    .iter()
                                    .any(|target| target.damage_amount == 0)
                            {
                                return Err(EngineError::Illegal(
                                    "damage allocations must be positive and sum to the total damage",
                                ));
                            }
                        }
                        if selection.targets.len() > 1 {
                            extra_generic +=
                                *extra_mana_per_target * (selection.targets.len() as u32 - 1);
                        }
                    }
                }
                public_targets.extend(selection.targets.iter().cloned());
                chosen_mode_indices.push(selection.mode_index);
                chosen_mode_labels.push(mode.label.clone());
                chosen_modes.push(ChosenMode {
                    mode_index: selection.mode_index as usize,
                    targets: selection
                        .targets
                        .iter()
                        .map(|target| capture_stack_target(self, target))
                        .collect(),
                });
            }
        } else {
            if !selected_modes.is_empty() {
                return Err(EngineError::Illegal(
                    "selected_modes given for a nonmodal spell",
                ));
            }
            validate_spell_targets(
                self,
                player,
                target_source,
                &face_effects,
                face_targeting.as_ref(),
                targets,
            )?;
            for effect in &face_effects {
                if let SpellEffectKind::DamageTargets {
                    amount,
                    division,
                    extra_mana_per_target,
                    ..
                } = effect
                {
                    if matches!(division, DamageDivision::ChooseAtCast) {
                        let allocated: u32 =
                            targets.iter().map(|target| target.damage_amount).sum();
                        if allocated
                            != amount.resolve_unconditional(chosen_x).ok_or(
                                EngineError::Illegal(
                                    "DamageTargets cannot use a conditional amount",
                                ),
                            )?
                            || targets.iter().any(|target| target.damage_amount == 0)
                        {
                            return Err(EngineError::Illegal(
                                "damage allocations must be positive and sum to the total damage",
                            ));
                        }
                    }
                    if targets.len() > 1 {
                        extra_generic += *extra_mana_per_target * (targets.len() as u32 - 1);
                    }
                }
            }
            public_targets.extend_from_slice(targets);
        }

        extra_generic = extra_generic.saturating_add(self.targeting_cost_increase(
            player,
            TargetingCostAction::Spells,
            &public_targets,
        ));

        let payment_plan = self.plan_spell_costs(
            player,
            idx,
            oid,
            &face_mana,
            chosen_x,
            extra_generic,
            self.spell_generic_reduction(player, oid, &cost_modifiers),
            flex_payments,
            &additional_costs,
            cost_selections,
            &cast_cost_groups,
            cast_cost_group_selections,
            restricted_mana,
            &eligible_restricted_mana,
            cast_method,
        )?;

        let trefs: Vec<ObjectId> = public_targets
            .iter()
            .map(|target| target.object_id)
            .collect();
        let stack_generation = self
            .state
            .zone_change_generation
            .get(&oid)
            .copied()
            .unwrap_or(0)
            .saturating_add(1);
        // CR 115.9: target-watchers see the final legal target set. Collect now so the event's
        // battlefield identity is exact, but do not stage anything unless all costs are paid and
        // the spell is successfully cast.
        let mut target_triggers = self.collect_event_triggers(&[GameEvent::TargetsChosen {
            controller: player,
            source: TargetingSourceKind::SpellCast,
            stack_object: StackObjectRef {
                object_id: oid,
                zone_change_generation: Some(stack_generation),
            },
            targets: trefs.clone(),
        }]);
        let payment = self.commit_cost_transaction(payment_plan)?;
        let life_paid = payment.life_paid;
        let paid_costs_line = format_paid_card_costs_log(&payment.paid_card_costs);
        let payment_result = CardResultCohort {
            cards: payment
                .paid_card_costs
                .iter()
                .map(|cost| cost.result().clone())
                .collect(),
        };
        let cast_cost_receipts = payment.cast_cost_receipts;
        let chosen_cast_cost_labels = cast_cost_receipts
            .iter()
            .map(|receipt| receipt.label.clone())
            .collect::<Vec<_>>();

        let stack_targets = public_targets
            .iter()
            .map(|target| capture_stack_target(self, target))
            .collect();
        let tgt_line = format_spell_targets_log(&self.state, self.registry, &trefs);

        self.state.stack.push(StackItem {
            id: oid,
            controller: player,
            card_id: card_id.clone(),
            targets: stack_targets,
            ability_text: None,
            source_permanent_id: None,
            source_zone_change: 0,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            chosen_x,
            face_index,
            chosen_modes,
            cast_cost_receipts,
            payment_result,
            resolution_branch_choices: Default::default(),
            // A spell's effects always act on its controller.
            trigger_context: TriggerContext::default(),
            cast_method,
        });
        super::resolution::move_object_to_zone(
            &mut self.state,
            self.registry,
            oid,
            Zone::Stack,
            None,
        )?;

        self.state.passes_since_stack_change = 0;
        self.state.priority_idx = idx;

        let cast_card_id = card_id.clone();
        let mut batch = RuledEventBatch::default();
        let x_line = if has_x {
            format!(" (X={chosen_x})")
        } else {
            String::new()
        };
        let modes_line = if chosen_mode_labels.is_empty() {
            String::new()
        } else {
            format!(" [{}]", chosen_mode_labels.join("; "))
        };
        batch.events.push(ev_log(format!(
            "P{} casts {}{}{}{}{}",
            player, face_name, modes_line, x_line, paid_costs_line, tgt_line
        )));
        let mut stack_annotation = match (has_multiple_cast_options, has_x) {
            (true, true) => format!("{face_name} (X = {chosen_x})"),
            (true, false) => face_name.clone(),
            (false, true) => format!("X = {chosen_x}"),
            (false, false) => String::new(),
        };
        if !chosen_mode_labels.is_empty() {
            stack_annotation = chosen_mode_labels.join("\n");
        }
        if !chosen_cast_cost_labels.is_empty() {
            let costs = chosen_cast_cost_labels.join("\n");
            stack_annotation = if stack_annotation.is_empty() {
                costs
            } else {
                format!("{stack_annotation}\n{costs}")
            };
        }
        // Alternative cast methods are not necessarily printed on the displayed face. Prepend an
        // authoritative annotation so every player can see the method that governs stack exit.
        if let Some(method_label) = cast_method.label() {
            stack_annotation = if stack_annotation.is_empty() {
                method_label.to_string()
            } else {
                format!("{method_label}\n{stack_annotation}")
            };
        }
        if life_paid > 0 {
            batch.events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: player,
                    new_total: self.state.players[idx].life,
                    delta: -(life_paid as i32),
                })),
            });
            batch.events.push(ev_log(format!(
                "P{player} pays {life_paid} life (Phyrexian mana)."
            )));
        }
        for ev in payment.move_events {
            batch.events.push(ev);
        }
        batch.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                object_id: oid,
                description: face_name.clone(),
                targets: public_targets,
                ability_annotation: stack_annotation,
                card_id: cast_card_id.clone(),
                is_copy: false,
                is_triggered: false,
                copy_source_object_id: 0,
                chosen_mode_indices,
                chosen_mode_labels,
                chosen_cast_cost_labels,
            })),
        });
        let ordinal = self.record_spell_cast(player);
        target_triggers.extend(self.collect_committed_sacrifice_cost_dies(payment.sacrificed));
        target_triggers.extend(self.collect_event_triggers(&[GameEvent::SpellCast {
            caster: player,
            card_id: cast_card_id,
            ordinal,
            face_index,
        }]));
        // Both kinds of triggers are waiting when the cast completes, so they form one CR 603.3b
        // ordering group rather than forcing an artificial target-trigger/cast-trigger order.
        self.stage_triggers(target_triggers);
        batch.events.push(ev_priority_changed(self));
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    pub(super) fn active_mana_options<'a>(
        &self,
        permanent_id: ObjectId,
        ability: &'a tricerules_cards::ActivatedAbilityDef,
    ) -> Option<&'a [tricerules_cards::ManaAmount]> {
        let default_options = ability.mana_options()?;
        let controller = self.state.objects.get(&permanent_id)?.controller;
        Some(
            ability
                .conditional_mana_output()
                .filter(|conditional| {
                    self.condition_holds(
                        &conditional.condition,
                        ConditionContext {
                            controller,
                            source_object_id: permanent_id,
                            source_zone_change: self
                                .state
                                .zone_change_generation
                                .get(&permanent_id)
                                .copied()
                                .unwrap_or(0),
                            resolving_spell_id: None,
                        },
                    )
                })
                .map(|conditional| conditional.options.as_slice())
                .unwrap_or(default_options.as_slice()),
        )
    }

    /// Printed activated abilities available from a nonbattlefield zone. Ability indices flatten
    /// printed faces in order while preserving slots for abilities authored for other zones.
    pub(super) fn authored_zone_activated_abilities(
        &self,
        source_id: ObjectId,
        source_zone: AbilitySourceZone,
    ) -> Vec<(usize, ActivatedAbilityDef, usize)> {
        let Some(definition) = self
            .state
            .objects
            .get(&source_id)
            .and_then(|object| self.registry.get(&object.card_id))
        else {
            return Vec::new();
        };
        let faces: Vec<_> = if definition.layout == Layout::Split {
            definition.faces_iter().enumerate().collect()
        } else {
            vec![(0, definition.primary_face())]
        };
        let mut next_index = 0usize;
        let mut result = Vec::new();
        for (face_index, face) in faces {
            for ability in &face.activated_abilities {
                let ability_index = next_index;
                next_index += 1;
                if ability.source_zone == source_zone {
                    result.push((ability_index, ability.clone(), face_index));
                }
            }
        }
        result
    }

    pub(super) fn activate_ability(
        &mut self,
        player: PlayerId,
        command: &rv1::ActivateAbility,
    ) -> Result<RuledEventBatch, EngineError> {
        let permanent_id = command.source_object_id;
        let source_zone = match rv1::AbilitySourceZone::try_from(command.source_zone)
            .map_err(|_| EngineError::Illegal("unknown ability source zone"))?
        {
            rv1::AbilitySourceZone::Battlefield => AbilitySourceZone::Battlefield,
            rv1::AbilitySourceZone::Hand => AbilitySourceZone::Hand,
            rv1::AbilitySourceZone::Graveyard => AbilitySourceZone::Graveyard,
        };
        let ability_index = command.ability_index as usize;
        let targets = command.targets.as_slice();
        let flex_payments = command.flex_payments.as_slice();
        let mana_option_index = command.mana_option_index;
        let cost_selections = command.cost_selections.as_slice();
        let restricted_mana = command.restricted_mana.as_slice();
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        if self.state.turn_step == TurnStep::Cleanup {
            return Err(EngineError::Illegal("no abilities during cleanup"));
        }
        if priority_locked_for_combat_declaration(&self.state) {
            return Err(EngineError::Illegal(
                "cannot activate until attack or block declaration is complete",
            ));
        }
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;

        let object = self
            .state
            .objects
            .get(&permanent_id)
            .ok_or(EngineError::Illegal("ability source missing"))?;
        let expected_zone = match source_zone {
            AbilitySourceZone::Battlefield => Zone::Battlefield,
            AbilitySourceZone::Hand => Zone::Hand,
            AbilitySourceZone::Graveyard => Zone::Graveyard,
        };
        let in_player_zone = match source_zone {
            AbilitySourceZone::Battlefield => {
                object.controller == player
                    && self.state.players[idx].battlefield.contains(&permanent_id)
            }
            AbilitySourceZone::Hand => {
                object.owner == player && self.state.players[idx].hand.contains(&permanent_id)
            }
            AbilitySourceZone::Graveyard => {
                object.owner == player && self.state.players[idx].graveyard.contains(&permanent_id)
            }
        };
        if object.zone != expected_zone || !in_player_zone {
            return Err(EngineError::Illegal(
                "ability source is not in its authored zone",
            ));
        }
        let source_zone_change = self
            .state
            .zone_change_generation
            .get(&permanent_id)
            .copied()
            .unwrap_or(0);
        if source_zone_change != command.expected_zone_change_generation {
            return Err(EngineError::Illegal("stale ability source generation"));
        }

        let (card_id, face_up_index, ability) = match source_zone {
            AbilitySourceZone::Battlefield => {
                let (card_id, face_index) = self
                    .effective_card_identity(permanent_id)
                    .map(|(card_id, face_index)| (card_id.to_string(), face_index))
                    .ok_or(EngineError::Illegal("bad face index on permanent"))?;
                let ability = self
                    .effective_activated_abilities(permanent_id)
                    .into_iter()
                    .find(|(index, _, _)| *index == ability_index)
                    .map(|(_, ability, _)| ability)
                    .ok_or(EngineError::Illegal("no such activated ability"))?;
                (card_id, face_index, ability)
            }
            AbilitySourceZone::Hand | AbilitySourceZone::Graveyard => {
                let card_id = object.card_id.clone();
                let (ability, face_index) = self
                    .authored_zone_activated_abilities(permanent_id, source_zone)
                    .into_iter()
                    .find(|(index, _, _)| *index == ability_index)
                    .map(|(_, ability, face_index)| (ability, face_index))
                    .ok_or(EngineError::Illegal("no such activated ability"))?;
                (card_id, face_index, ability)
            }
        };
        let resolving_mana_payment =
            self.state
                .pending_resolution
                .as_ref()
                .is_some_and(|pending| {
                    pending.continuation.mana_payment().is_some()
                        && pending.deciding_player == player
                });
        if resolving_mana_payment {
            if ability.mana_options().is_none() {
                return Err(EngineError::Illegal(
                    "only mana abilities may be activated during a resolution payment",
                ));
            }
        } else if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        if source_zone != AbilitySourceZone::Battlefield && ability.mana_options().is_some() {
            return Err(EngineError::Illegal(
                "nonbattlefield mana abilities are not supported",
            ));
        }

        // CR 602.5d / 702.6a: explicit activation instructions and equip both use the shared
        // sorcery-speed window.
        if ability.requires_sorcery_speed()
            && !super::priority::sorcery_speed_available(&self.state, player)
        {
            return Err(EngineError::Illegal("ability only at sorcery speed"));
        }
        if !self.activation_conditions_hold(permanent_id, &ability) {
            return Err(EngineError::Illegal("activation condition not met"));
        }
        if !self.activation_limit_allows(permanent_id, ability_index, &ability) {
            return Err(EngineError::Illegal("activation limit reached"));
        }

        if ability.mana_options().is_some() {
            let mut batch = self.resolve_mana_ability(
                player,
                idx,
                permanent_id,
                ability_index,
                &card_id,
                &ability,
                mana_option_index,
                targets,
                flex_payments,
                cost_selections,
                restricted_mana,
            )?;
            if resolving_mana_payment {
                batch.events.push(
                    self.resolution_payment_choice_event()
                        .expect("resolution payment remains parked after a mana ability"),
                );
            }
            return Ok(batch);
        }

        self.state.undoable_mana_abilities.clear();

        // CR 608.2: an ability's effect list validates exactly like a spell's — same shape,
        // same multi-target handling — so it goes through the list-level entry point.
        let target_source = TargetSourceIdentity::captured(permanent_id, source_zone_change);
        validate_ability_targets(
            self,
            player,
            target_source,
            &ability.effect,
            ability.targeting.as_ref(),
            targets,
        )?;

        let trefs: Vec<ObjectId> = targets.iter().map(|t| t.object_id).collect();
        // Reserve without consuming: a failed payment must not advance the deterministic id
        // stream, while target triggers still need the eventual ability's exact identity.
        let virtual_id = self.state.next_object_id;
        // Snapshot target-watchers before costs: the source itself can be sacrificed while paying
        // for the activation. Nothing is staged unless payment succeeds and the ability is pushed.
        let mut target_triggers = self.collect_event_triggers(&[GameEvent::TargetsChosen {
            controller: player,
            source: TargetingSourceKind::Ability,
            stack_object: StackObjectRef {
                object_id: virtual_id,
                zone_change_generation: None,
            },
            targets: trefs.clone(),
        }]);

        let source_face_change = self
            .state
            .face_change_generation
            .get(&permanent_id)
            .copied()
            .unwrap_or(0);
        let targeting_cost =
            self.targeting_cost_increase(player, TargetingCostAction::ActivatedAbilities, targets);
        let activation_use_key = ability
            .activation_limit
            .map(|_| self.activation_use_key(permanent_id, ability_index));
        let cost_plan = self.plan_ability_costs(
            player,
            idx,
            permanent_id,
            &ability.costs,
            flex_payments,
            cost_selections,
            restricted_mana,
            targeting_cost,
        )?;
        let payment = self.commit_cost_transaction(cost_plan)?;
        self.record_limited_activation(activation_use_key);

        let ability_text = ability.text.clone();
        let card_name = self
            .registry
            .get(&card_id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| card_id.clone());

        self.state.next_object_id += 1;

        self.state.stack.push(StackItem {
            id: virtual_id,
            controller: player,
            card_id: card_id.clone(),
            targets: targets
                .iter()
                .map(|target| capture_stack_target(self, target))
                .collect(),
            ability_text: Some(ability_text.clone()),
            source_permanent_id: Some(permanent_id),
            source_zone_change,
            source_face_change,
            ability_index: Some(ability_index),
            activated_ability: Some(ability.clone()),
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            chosen_x: 0,
            face_index: face_up_index,
            chosen_modes: vec![],
            cast_cost_receipts: vec![],
            payment_result: CardResultCohort {
                cards: payment
                    .paid_card_costs
                    .iter()
                    .map(|cost| cost.result().clone())
                    .collect(),
            },
            resolution_branch_choices: Default::default(),
            // An activated ability's effects act on the player who activated it.
            trigger_context: TriggerContext::default(),
            cast_method: SpellCastMethod::Normal,
        });
        self.state.passes_since_stack_change = 0;
        self.state.priority_idx = idx;

        let tgt_line = format_spell_targets_log(&self.state, self.registry, &trefs);
        let paid_costs_line = format_paid_card_costs_log(&payment.paid_card_costs);
        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!(
            "P{player} activates {card_name}{paid_costs_line}: {ability_text}{tgt_line}"
        )));
        if payment.life_paid > 0 {
            batch.events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: player,
                    new_total: self.state.players[idx].life,
                    delta: -(payment.life_paid as i32),
                })),
            });
            batch.events.push(ev_log(format!(
                "P{player} pays {} life (Phyrexian mana).",
                payment.life_paid
            )));
        }
        for ev in payment.move_events {
            batch.events.push(ev);
        }
        batch.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                object_id: virtual_id,
                description: card_name,
                targets: targets.to_vec(),
                ability_annotation: ability_text,
                card_id: String::new(),
                is_copy: false,
                is_triggered: false,
                copy_source_object_id: 0,
                chosen_mode_indices: vec![],
                chosen_mode_labels: vec![],
                chosen_cast_cost_labels: vec![],
            })),
        });
        // CR 603.3b: a trigger that fired while the cost was being paid goes on the stack *above*
        // the ability it paid for, so its events must follow that ability's StackPushed. Emitting
        // the trigger prompt first also made the client discard it, because an activated ability
        // reaching the stack is its signal that a pending trigger target was just answered.
        target_triggers.extend(self.collect_committed_sacrifice_cost_dies(payment.sacrificed));
        self.stage_triggers(target_triggers);
        batch.events.push(ev_priority_changed(self));
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    /// CR 603.6a: a permanent sacrificed to pay an activation cost still dies, so leaves-the-
    /// battlefield abilities (Blood Artist, Bottle Gnomes' own controller triggers) see it. The
    /// triggers go on the stack *above* the ability whose cost they paid, so this runs after the
    /// ability has been pushed.
    fn collect_committed_sacrifice_cost_dies(
        &mut self,
        snapshots: Vec<SacrificeSnapshot>,
    ) -> Vec<super::triggers::CollectedTrigger> {
        let events: Vec<_> = snapshots
            .into_iter()
            .map(|snapshot| GameEvent::Dies {
                source: snapshot.source,
                was_creature: snapshot.was_creature,
            })
            .collect();
        self.record_committed_events(&events);
        self.collect_event_triggers(&events)
    }

    fn fire_sacrifice_cost_dies(&mut self, snapshots: Vec<SacrificeSnapshot>) {
        let collected = self.collect_committed_sacrifice_cost_dies(snapshots);
        self.stage_triggers(collected);
    }

    /// Whether `ability` on `permanent_id` could be activated right now, so the client can grey it
    /// out instead of opening a menu and collecting mana for an activation the engine will reject.
    ///
    /// Deliberately ignores mana: the client floats mana *after* choosing the ability, so an
    /// unaffordable ability is still a legal thing to start. What it does mirror is every gate in
    /// [`GameEngine::activate_ability`] that no amount of mana can satisfy — the tap cost
    /// (CR 302.6 summoning sickness, already tapped), public activation conditions, and stricter
    /// timing instructions (CR 602.5d / 702.6a). Priority and seat ownership stay client-side;
    /// they are not properties of the permanent, and this value is broadcast to every player.
    pub(super) fn ability_activatable(
        &self,
        permanent_id: ObjectId,
        ability_index: usize,
        ability: &tricerules_cards::ActivatedAbilityDef,
    ) -> bool {
        let Some(object) = self.state.objects.get(&permanent_id) else {
            return false;
        };
        let expected_zone = match ability.source_zone {
            AbilitySourceZone::Battlefield => Zone::Battlefield,
            AbilitySourceZone::Hand => Zone::Hand,
            AbilitySourceZone::Graveyard => Zone::Graveyard,
        };
        if object.zone != expected_zone {
            return false;
        }
        let activating_player = if ability.source_zone == AbilitySourceZone::Battlefield {
            object.controller
        } else {
            object.owner
        };
        if ability.requires_sorcery_speed()
            && !super::priority::sorcery_speed_available(&self.state, activating_player)
        {
            return false;
        }
        if !self.activation_conditions_hold(permanent_id, ability) {
            return false;
        }
        if !self.activation_limit_allows(permanent_id, ability_index, ability) {
            return false;
        }
        if ability
            .costs
            .iter()
            .any(|cost| matches!(cost, AbilityCost::Tap))
            && self.check_tappable(permanent_id, &object.card_id).is_err()
        {
            return false;
        }
        true
    }

    fn activation_use_key(&self, permanent_id: ObjectId, ability_index: usize) -> ActivationUseKey {
        ActivationUseKey {
            object_id: permanent_id,
            zone_change_generation: self
                .state
                .zone_change_generation
                .get(&permanent_id)
                .copied()
                .unwrap_or(0),
            face_change_generation: self
                .state
                .face_change_generation
                .get(&permanent_id)
                .copied()
                .unwrap_or(0),
            ability_index,
        }
    }

    fn activation_limit_allows(
        &self,
        permanent_id: ObjectId,
        ability_index: usize,
        ability: &tricerules_cards::ActivatedAbilityDef,
    ) -> bool {
        let Some(limit) = ability.activation_limit else {
            return true;
        };
        let key = self.activation_use_key(permanent_id, ability_index);
        self.state
            .activation_uses_this_turn
            .get(&key)
            .copied()
            .unwrap_or(0)
            < limit.max_activations()
    }

    fn record_limited_activation(&mut self, key: Option<ActivationUseKey>) {
        let Some(key) = key else {
            return;
        };
        let count = self.state.activation_uses_this_turn.entry(key).or_insert(0);
        *count = count.saturating_add(1);
    }

    fn activation_conditions_hold(
        &self,
        permanent_id: ObjectId,
        ability: &tricerules_cards::ActivatedAbilityDef,
    ) -> bool {
        let Some(controller) = self.state.objects.get(&permanent_id).map(|object| {
            if ability.source_zone == AbilitySourceZone::Battlefield {
                object.controller
            } else {
                object.owner
            }
        }) else {
            return false;
        };
        ability.conditions.iter().all(|condition| match condition {
            tricerules_cards::ActivationCondition::GameCondition(condition) => self
                .condition_holds(
                    condition,
                    ConditionContext {
                        controller,
                        source_object_id: permanent_id,
                        source_zone_change: self
                            .state
                            .zone_change_generation
                            .get(&permanent_id)
                            .copied()
                            .unwrap_or(0),
                        resolving_spell_id: None,
                    },
                ),
            tricerules_cards::ActivationCondition::BattlefieldCreatureCount {
                filter,
                min,
                max,
            } => {
                let count = self.battlefield_creature_count(filter, controller, permanent_id);
                min.is_none_or(|minimum| count >= minimum)
                    && max.is_none_or(|maximum| count <= maximum)
            }
        })
    }

    pub(super) fn check_tappable(
        &self,
        permanent_id: ObjectId,
        _card_id: &str,
    ) -> Result<(), EngineError> {
        let has_haste = self
            .characteristics(permanent_id)
            .is_some_and(|value| value.has_keyword(tricerules_cards::Keyword::Haste));
        let o = self
            .state
            .objects
            .get(&permanent_id)
            .ok_or(EngineError::Illegal("permanent missing"))?;
        if o.tapped {
            return Err(EngineError::Illegal("permanent is already tapped"));
        }
        let is_creature = self
            .characteristics(permanent_id)
            .is_some_and(|value| value.is_creature());
        if is_creature && o.summoning_sick && !has_haste {
            return Err(EngineError::Illegal(
                "cannot use tap ability due to summoning sickness",
            ));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn resolve_mana_ability(
        &mut self,
        player: PlayerId,
        idx: usize,
        permanent_id: ObjectId,
        ability_index: usize,
        card_id: &str,
        ability: &tricerules_cards::ActivatedAbilityDef,
        mana_option_index: u32,
        targets: &[rv1::TargetRef],
        flex_payments: &[rv1::FlexPipPayment],
        cost_selections: &[rv1::CostSelection],
        restricted_mana: &[rv1::ManaSpendSelection],
    ) -> Result<RuledEventBatch, EngineError> {
        if !targets.is_empty() {
            return Err(EngineError::Illegal("mana ability takes no targets"));
        }
        let Some(options) = self.active_mana_options(permanent_id, ability) else {
            return Err(EngineError::Illegal("not a mana ability"));
        };
        let amount = options
            .get(mana_option_index as usize)
            .copied()
            .ok_or(EngineError::Illegal("invalid mana option"))?;
        let activation_use_key = ability
            .activation_limit
            .map(|_| self.activation_use_key(permanent_id, ability_index));

        let cost_plan = self.plan_ability_costs(
            player,
            idx,
            permanent_id,
            &ability.costs,
            flex_payments,
            cost_selections,
            restricted_mana,
            0,
        )?;
        let payment = self.commit_cost_transaction(cost_plan)?;
        self.record_limited_activation(activation_use_key);

        let restriction_group_id = ability.mana_restriction().map(|restriction| {
            if let Some(position) = self
                .state
                .mana_restrictions
                .iter()
                .position(|candidate| candidate == restriction)
            {
                (position + 1) as u32
            } else {
                self.state.mana_restrictions.push(restriction.clone());
                self.state.mana_restrictions.len() as u32
            }
        });
        if let Some(group_id) = restriction_group_id {
            self.state.players[idx].restricted_mana.push(
                crate::state::RestrictedManaContribution {
                    restriction_group_id: group_id,
                    amount,
                },
            );
        } else {
            let pool = &mut self.state.players[idx].mana_pool;
            pool.white += amount.w;
            pool.blue += amount.u;
            pool.black += amount.b;
            pool.red += amount.r;
            pool.green += amount.g;
            pool.colorless += amount.c;
        }

        if matches!(ability.costs.as_slice(), [AbilityCost::Tap]) {
            self.state
                .undoable_mana_abilities
                .push(UndoableManaAbility {
                    player,
                    source: permanent_id,
                    produced: amount,
                    restriction_group_id,
                });
        }

        let card_name = self
            .registry
            .get(card_id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| card_id.to_string());
        let ability_text = ability.text.clone();
        let paid_costs_line = format_paid_card_costs_log(&payment.paid_card_costs);

        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!(
            "P{player} activates {card_name}{paid_costs_line}: {ability_text}"
        )));
        for ev in payment.move_events {
            batch.events.push(ev);
        }
        if payment.life_paid > 0 {
            batch.events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: player,
                    new_total: self.state.players[idx].life,
                    delta: -(payment.life_paid as i32),
                })),
            });
            batch.events.push(ev_log(format!(
                "P{player} pays {} life (Phyrexian mana).",
                payment.life_paid
            )));
        }
        // A mana ability does not use the stack (CR 605.3a), so a permanent sacrificed to pay for
        // one dies immediately rather than under a pushed ability.
        self.fire_sacrifice_cost_dies(payment.sacrificed);
        Ok(batch)
    }

    pub(super) fn undo_mana_ability(
        &mut self,
        player: PlayerId,
    ) -> Result<RuledEventBatch, EngineError> {
        let payment_undo_start = self.state.pending_resolution.as_ref().and_then(|pending| {
            (pending.deciding_player == player)
                .then_some(pending.continuation.mana_payment()?.undo_history_start)
        });
        if self.state.priority_player_id() != player && payment_undo_start.is_none() {
            return Err(EngineError::Illegal("not your priority"));
        }
        let event =
            self.rewind_last_undoable_mana_ability(player, payment_undo_start.unwrap_or(0))?;

        let mut batch = RuledEventBatch::default();
        batch.events.push(event);
        if payment_undo_start.is_some() {
            batch.events.push(
                self.resolution_payment_choice_event()
                    .expect("resolution payment remains parked after undo"),
            );
        }
        Ok(batch)
    }

    pub(super) fn rewind_last_undoable_mana_ability(
        &mut self,
        player: PlayerId,
        first_allowed_index: usize,
    ) -> Result<rv1::RuledEvent, EngineError> {
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let pos = self
            .state
            .undoable_mana_abilities
            .iter()
            .enumerate()
            .rfind(|(index, entry)| *index >= first_allowed_index && entry.player == player)
            .map(|(index, _)| index)
            .ok_or(EngineError::Illegal("no mana ability to undo"))?;
        let entry = self.state.undoable_mana_abilities.remove(pos);

        let p = &entry.produced;
        if let Some(group_id) = entry.restriction_group_id {
            let contribution_pos = self.state.players[idx]
                .restricted_mana
                .iter()
                .rposition(|contribution| {
                    contribution.restriction_group_id == group_id && contribution.amount == *p
                })
                .ok_or(EngineError::Illegal(
                    "floated restricted mana already spent",
                ))?;
            self.state.players[idx]
                .restricted_mana
                .remove(contribution_pos);
        } else {
            let pool = &mut self.state.players[idx].mana_pool;
            if pool.white < p.w
                || pool.blue < p.u
                || pool.black < p.b
                || pool.red < p.r
                || pool.green < p.g
                || pool.colorless < p.c
            {
                return Err(EngineError::Illegal("floated mana already spent"));
            }
            pool.white -= p.w;
            pool.blue -= p.u;
            pool.black -= p.b;
            pool.red -= p.r;
            pool.green -= p.g;
            pool.colorless -= p.c;
        }

        super::set_tapped(&mut self.state, entry.source, false);

        let card_name = self
            .state
            .objects
            .get(&entry.source)
            .and_then(|o| self.registry.get(&o.card_id))
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "permanent".to_string());
        Ok(ev_log(format!(
            "P{player} undoes mana ability: {card_name}"
        )))
    }

    pub(super) fn play_land(
        &mut self,
        player: PlayerId,
        command: &rv1::PlayLand,
    ) -> Result<RuledEventBatch, EngineError> {
        let face_index = command.face_index as usize;
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        let max_lands = 1 + self.extra_land_plays_for(player);
        if self.state.lands_played_this_turn >= max_lands {
            return Err(EngineError::Illegal(
                "land play limit reached for this turn",
            ));
        }
        if !super::priority::sorcery_speed_available(&self.state, player) {
            return Err(EngineError::Illegal("play land only at sorcery speed"));
        }
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let source = command
            .source
            .as_ref()
            .and_then(|source| source.location.as_ref())
            .ok_or(EngineError::Illegal("missing land source"))?;
        let (oid, from_hand) = match source {
            rv1::land_source::Location::HandIndex(hand_index) => (
                *self.state.players[idx]
                    .hand
                    .get(*hand_index as usize)
                    .ok_or(EngineError::Illegal("bad hand index"))?,
                true,
            ),
            rv1::land_source::Location::ExileObjectId(object_id) => {
                let object = self
                    .state
                    .objects
                    .get(object_id)
                    .ok_or(EngineError::Illegal("unknown exile object"))?;
                let generation = self
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0);
                let permitted = self
                    .state
                    .active_exile_play_permissions
                    .iter()
                    .any(|permission| {
                        permission.player_id == player
                            && permission.object_id == *object_id
                            && permission.zone_change_generation == generation
                            && permission.scope == ExilePlayPermissionScope::PlayCard
                    });
                if object.zone != Zone::Exile || !permitted {
                    return Err(EngineError::Illegal("illegal land play from exile"));
                }
                (*object_id, false)
            }
        };
        let card_id = self.state.objects.get(&oid).unwrap().card_id.clone();
        let def = self
            .registry
            .get(&card_id)
            .ok_or_else(|| EngineError::MissingCard(card_id.clone()))?;
        // CR 712: for MDFC lands, check the specific face; for normal lands, check the flat flag.
        let face = def
            .face(face_index)
            .ok_or(EngineError::Illegal("bad face index"))?;
        if from_hand && !def.face_available_from_hand(face_index) {
            return Err(EngineError::Illegal("face cannot be played from hand"));
        }
        if !face.is_land {
            return Err(EngineError::Illegal("not a land"));
        }
        let land_name = if def.is_multiface() {
            face.name.to_string()
        } else {
            def.name.clone()
        };
        self.state.lands_played_this_turn += 1;
        let mut batch = RuledEventBatch::default();
        let item = StackItem {
            id: oid,
            controller: player,
            card_id,
            targets: Vec::new(),
            ability_text: Some("land play".to_string()),
            source_permanent_id: None,
            source_zone_change: self
                .state
                .zone_change_generation
                .get(&oid)
                .copied()
                .unwrap_or(0),
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index,
            cast_method: SpellCastMethod::Normal,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            cast_cost_receipts: Vec::new(),
            payment_result: CardResultCohort::default(),
            resolution_branch_choices: Default::default(),
            trigger_context: TriggerContext::default(),
        };
        match self.begin_battlefield_entry(
            item,
            BattlefieldEntryEvent {
                object_id: oid,
                deciding_player: player,
                destination_controller: player,
                face_index,
                unlock_room_door: None,
                chosen_x: 0,
                cast_cost_receipts: Vec::new(),
                player_life_snapshot: self.player_life_snapshot(),
                tapped: false,
                entry_counters: BTreeMap::new(),
                applied_effects: Vec::new(),
            },
            BattlefieldEntryCompletion::LandPlay {
                player,
                land_name: land_name.clone(),
            },
            &mut batch.events,
        ) {
            super::replacement::BattlefieldEntryProgress::Parked => return Ok(batch),
            super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                self.commit_battlefield_entry(entry, None)?;
            }
        }
        self.state.passes_since_stack_change = 0;
        batch
            .events
            .push(ev_log(format!("P{player} played {land_name}")));
        fill_legal(&mut batch, self);
        Ok(batch)
    }
}

#[cfg(test)]
mod mana_payment_tests {
    use super::*;

    /// Build a 2-player engine and hand priority to player 0 so `pay_mana`'s priority gate passes.
    fn engine_with_priority() -> GameEngine {
        let mut e = GameEngine::new_with_default_decks(1, &[0, 1], 20).expect("new");
        e.state.priority_idx = 0;
        e
    }

    #[test]
    fn conditional_instant_timing_requires_the_linked_cast_cost_selection() {
        let condition = CastCostReceiptCondition {
            group_index: 0,
            option_index: 1,
            expected_selected: true,
        };
        assert!(!command_satisfies_cast_cost_condition(&[], condition));
        assert!(!command_satisfies_cast_cost_condition(
            &[rv1::CastCostGroupSelection {
                group_index: 0,
                option_index: 0,
                selected_object: None,
                expected_zone_change_generation: 0,
            }],
            condition,
        ));
        assert!(command_satisfies_cast_cost_condition(
            &[rv1::CastCostGroupSelection {
                group_index: 0,
                option_index: 1,
                selected_object: None,
                expected_zone_change_generation: 0,
            }],
            condition,
        ));
    }

    #[test]
    fn paid_card_cost_log_formats_any_number_of_components() {
        let result = |action, object_id| CardResultEntry {
            action,
            affected_player: 0,
            object_id,
            zone_change_generation: 1,
            matched_card_types: Vec::new(),
        };
        assert_eq!(format_paid_card_costs_log(&[]), "");
        assert_eq!(
            format_paid_card_costs_log(&[PaidCardCost::Discard {
                object_id: 1,
                card_name: "Mountain".into(),
                result: result(CardResultAction::Discard, 1),
            }]),
            " discarding Mountain"
        );
        assert_eq!(
            format_paid_card_costs_log(&[
                PaidCardCost::Discard {
                    object_id: 1,
                    card_name: "Mountain".into(),
                    result: result(CardResultAction::Discard, 1),
                },
                PaidCardCost::Sacrifice {
                    object_id: 2,
                    card_name: "Grizzly Bears".into(),
                    result: result(CardResultAction::Sacrifice, 2),
                },
            ]),
            " discarding Mountain and sacrificing Grizzly Bears"
        );
        assert_eq!(
            format_paid_card_costs_log(&[
                PaidCardCost::Discard {
                    object_id: 1,
                    card_name: "Mountain".into(),
                    result: result(CardResultAction::Discard, 1),
                },
                PaidCardCost::Sacrifice {
                    object_id: 2,
                    card_name: "Grizzly Bears".into(),
                    result: result(CardResultAction::Sacrifice, 2),
                },
                PaidCardCost::Sacrifice {
                    object_id: 3,
                    card_name: "Hill Giant".into(),
                    result: result(CardResultAction::Sacrifice, 3),
                },
            ]),
            " discarding Mountain, sacrificing Grizzly Bears, and sacrificing Hill Giant"
        );
    }

    #[test]
    fn activation_limit_counts_are_independent_per_effective_ability_index() {
        let mut engine = engine_with_priority();
        let object_id = engine.state.players[0].library[0];
        let ability = CardRegistry::global()
            .get("temur_devotee")
            .expect("Temur Devotee must be registered")
            .primary_face()
            .activated_abilities[0]
            .clone();

        assert!(engine.activation_limit_allows(object_id, 0, &ability));
        assert!(engine.activation_limit_allows(object_id, 1, &ability));
        let key = engine.activation_use_key(object_id, 0);
        engine.record_limited_activation(Some(key));
        assert!(!engine.activation_limit_allows(object_id, 0, &ability));
        assert!(engine.activation_limit_allows(object_id, 1, &ability));
    }

    #[test]
    fn limited_activation_records_the_pre_cost_object_identity() {
        let mut engine = engine_with_priority();
        let object_id = engine.state.players[0].library[0];
        let key_before_costs = engine.activation_use_key(object_id, 0);

        *engine
            .state
            .zone_change_generation
            .entry(object_id)
            .or_insert(0) += 1;
        engine.record_limited_activation(Some(key_before_costs));

        assert_eq!(engine.state.activation_uses_this_turn[&key_before_costs], 1);
        assert_ne!(engine.activation_use_key(object_id, 0), key_before_costs);
    }

    #[test]
    fn one_permanent_cannot_pay_two_consuming_cost_components() {
        use rv1::cost_selection::Selection;

        let mut e = engine_with_priority();
        let oid = e.state.players[0]
            .library
            .pop_front()
            .expect("default deck card");
        e.state.players[0].battlefield.push(oid);
        let object = e.state.objects.get_mut(&oid).expect("object");
        object.zone = Zone::Battlefield;
        object.base_controller = 0;
        object.controller = 0;
        let filter = TargetFilter {
            kind: TargetKind::AnyPermanent,
            controller: TargetController::You,
            ..TargetFilter::default()
        };
        let costs = [
            AbilityCost::SacrificePermanent {
                filter: filter.clone(),
            },
            AbilityCost::SacrificePermanent { filter },
        ];
        let selections = [
            rv1::CostSelection {
                cost_index: 0,
                selection: Some(Selection::PermanentId(oid)),
            },
            rv1::CostSelection {
                cost_index: 1,
                selection: Some(Selection::PermanentId(oid)),
            },
        ];

        let err = e
            .plan_ability_costs(0, 0, oid, &costs, &[], &selections, &[], 0)
            .err()
            .expect("one object cannot be sacrificed twice");
        assert!(format!("{err:?}").contains("one object cannot pay two costs"));
        assert_eq!(e.state.objects[&oid].zone, Zone::Battlefield);
    }

    #[test]
    fn bounded_graveyard_cost_is_distinct_source_excluding_and_atomic() {
        use rv1::cost_selection::Selection;

        let mut e = engine_with_priority();
        let namesake_card_id = e.state.objects[&e.state.players[0].library[0]]
            .card_id
            .clone();
        let cards: Vec<ObjectId> = e.state.players[0]
            .library
            .iter()
            .copied()
            .filter(|oid| e.state.objects[oid].card_id == namesake_card_id)
            .take(3)
            .collect();
        assert_eq!(cards.len(), 3, "default deck has three namesakes");
        let namesake = e
            .registry
            .get(&namesake_card_id)
            .expect("registered namesake")
            .name
            .clone();
        for &oid in &cards {
            e.state.players[0]
                .library
                .retain(|candidate| *candidate != oid);
            e.state.players[0].graveyard.push(oid);
            e.state.objects.get_mut(&oid).expect("object").zone = Zone::Graveyard;
        }
        let source = cards[0];
        let costs = [
            AbilityCost::ExileSelf,
            AbilityCost::ExileGraveyardCards {
                count: 2,
                filter: ZoneCardFilter {
                    exact_name: Some(namesake),
                    ..Default::default()
                },
                exclude_source: true,
            },
        ];
        let select = |ids| rv1::CostSelection {
            cost_index: 1,
            selection: Some(Selection::GraveyardObjectIds(rv1::GraveyardObjectIds {
                object_ids: ids,
            })),
        };

        let duplicate = [select(vec![cards[1], cards[1]])];
        assert!(e
            .plan_ability_costs(0, 0, source, &costs, &[], &duplicate, &[], 0)
            .is_err());
        assert!(cards
            .iter()
            .all(|oid| e.state.objects[oid].zone == Zone::Graveyard));

        let selected = [select(vec![cards[1], cards[2]])];
        let plan = e
            .plan_ability_costs(0, 0, source, &costs, &[], &selected, &[], 0)
            .expect("three distinct graveyard objects validate together");
        e.commit_cost_transaction(plan)
            .expect("costs commit atomically");
        assert!(cards
            .iter()
            .all(|oid| e.state.objects[oid].zone == Zone::Exile));
    }

    #[test]
    fn source_exclusion_is_preserved_through_disjunctive_cost_filters() {
        let mut e = engine_with_priority();
        let source = e.state.players[0].library.pop_front().expect("source card");
        let other = e.state.players[0].library.pop_front().expect("other card");
        for oid in [source, other] {
            e.state.players[0].battlefield.push(oid);
            let object = e.state.objects.get_mut(&oid).expect("object");
            object.zone = Zone::Battlefield;
            object.base_controller = 0;
            object.controller = 0;
        }
        let leaf = TargetFilter {
            kind: TargetKind::AnyPermanent,
            controller: TargetController::You,
            exclude_source: true,
            ..TargetFilter::default()
        };
        let filter = TargetFilter {
            any_of: Some(vec![leaf.clone(), leaf]),
            ..TargetFilter::default()
        };

        assert!(!e.ability_cost_permanent_matches(0, Some(source), source, &filter));
        assert!(e.ability_cost_permanent_matches(0, Some(source), other, &filter));
        assert!(e.ability_cost_permanent_matches(0, None, source, &filter));
    }

    #[test]
    fn multi_digit_generic_paid_from_pool() {
        // Regression for the old per-char parser: "{15}" must consume 15, not 6.
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 15;
        let cost = ManaCost {
            pips: vec![ManaSymbol::Generic(15)],
        };
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    }

    #[test]
    fn insufficient_generic_mana_rejected() {
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 14;
        let cost = ManaCost {
            pips: vec![ManaSymbol::Generic(15)],
        };
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 0, 0, &[]),
            Err(EngineError::Illegal(_))
        ));
    }

    #[test]
    fn generic_reduction_applies_after_increases_and_floors_without_touching_colored_pips() {
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.blue = 1;
        e.state.players[0].mana_pool.colorless = 4;
        let cost = ManaCost::parse("{1}{U}").expect("cost");

        let plan = plan_mana_payment_with_reduction(&e.state, 0, &cost, 0, 2, 3, &[])
            .expect("the reduction applies after the increase");
        commit_mana_payment(&mut e.state, 0, plan);
        assert_eq!(e.state.players[0].mana_pool.blue, 0);
        assert_eq!(e.state.players[0].mana_pool.colorless, 4);

        e.state.players[0].mana_pool.blue = 1;
        let colored_only = ManaCost::parse("{U}").expect("cost");
        let plan =
            plan_mana_payment_with_reduction(&e.state, 0, &colored_only, 0, 0, u32::MAX, &[])
                .expect("an oversized generic reduction floors at zero");
        commit_mana_payment(&mut e.state, 0, plan);
        assert_eq!(e.state.players[0].mana_pool.blue, 0);
        assert_eq!(e.state.players[0].mana_pool.colorless, 4);
    }

    #[test]
    fn reduced_mana_and_discard_cost_are_planned_and_committed_together() {
        use rv1::cost_selection::Selection;

        let mut e = engine_with_priority();
        let source_oid = e.state.players[0].hand[0];
        let discarded_oid = e.state.players[0].hand[1];
        e.state.players[0].mana_pool.blue = 1;
        let costs = [AdditionalCost::DiscardCard];
        let selections = [rv1::CostSelection {
            cost_index: 0,
            selection: Some(Selection::HandIndex(1)),
        }];

        let plan = e
            .plan_spell_costs(
                0,
                0,
                source_oid,
                &ManaCost::parse("{1}{U}").expect("cost"),
                0,
                0,
                1,
                &[],
                &costs,
                &selections,
                &[],
                &[],
                &[],
                &[],
                SpellCastMethod::Normal,
            )
            .expect("reduced mana and discard cost should validate together");
        let payment = e
            .commit_cost_transaction(plan)
            .expect("validated costs should commit");

        assert_eq!(e.state.players[0].mana_pool.blue, 0);
        assert_eq!(e.state.objects[&source_oid].zone, Zone::Hand);
        assert_eq!(e.state.objects[&discarded_oid].zone, Zone::Graveyard);
        assert_eq!(payment.paid_card_costs.len(), 1);
    }

    #[test]
    fn stale_card_debit_rejects_before_mana_or_life_is_committed() {
        use rv1::cost_selection::Selection;

        let mut e = engine_with_priority();
        let source_oid = e.state.players[0].hand[0];
        let discarded_oid = e.state.players[0].hand[1];
        e.state.players[0].mana_pool.blue = 1;
        e.state.players[0].mana_pool.colorless = 1;
        let costs = [AdditionalCost::DiscardCard];
        let selections = [rv1::CostSelection {
            cost_index: 0,
            selection: Some(Selection::HandIndex(1)),
        }];

        let plan = e
            .plan_spell_costs(
                0,
                0,
                source_oid,
                &ManaCost::parse("{1}{U}").expect("cost"),
                0,
                0,
                0,
                &[],
                &costs,
                &selections,
                &[],
                &[],
                &[],
                &[],
                SpellCastMethod::Normal,
            )
            .expect("costs initially validate");
        *e.state
            .zone_change_generation
            .entry(discarded_oid)
            .or_insert(0) += 1;

        let err = e
            .commit_cost_transaction(plan)
            .err()
            .expect("stale physical-card identity must reject the whole payment");
        assert!(format!("{err:?}").contains("cost transaction became stale"));
        assert_eq!(e.state.players[0].mana_pool.blue, 1);
        assert_eq!(e.state.players[0].mana_pool.colorless, 1);
        assert_eq!(e.state.players[0].life, 20);
        assert_eq!(e.state.objects[&discarded_oid].zone, Zone::Hand);
    }

    #[test]
    fn colorless_pip_requires_colorless_mana() {
        // {C} cannot be paid with colored mana (unlike generic).
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 5;
        let cost = ManaCost {
            pips: vec![ManaSymbol::C],
        };
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 0, 0, &[]),
            Err(EngineError::Illegal(_))
        ));
        e.state.players[0].mana_pool.colorless = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
    }

    #[test]
    fn x_cost_pays_chosen_value_as_generic() {
        // CR 107.3b: {X}{R} with X=4 needs 4 generic + 1 red.
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 4;
        e.state.players[0].mana_pool.red = 1;
        let cost = ManaCost {
            pips: vec![ManaSymbol::X, ManaSymbol::R],
        };
        assert!(pay_mana(&mut e.state, 0, &cost, 4, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.colorless, 0);
        assert_eq!(e.state.players[0].mana_pool.red, 0);
    }

    #[test]
    fn x_zero_pays_only_fixed_pips() {
        // CR 107.3: X=0 is a legal choice; only the {R} must be paid.
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 1;
        let cost = ManaCost {
            pips: vec![ManaSymbol::X, ManaSymbol::R],
        };
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
    }

    #[test]
    fn insufficient_mana_for_chosen_x_rejected() {
        // Choosing X larger than the pool can cover fails cleanly (no partial payment).
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 3;
        e.state.players[0].mana_pool.red = 1;
        let cost = ManaCost {
            pips: vec![ManaSymbol::X, ManaSymbol::R],
        };
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 4, 0, &[]),
            Err(EngineError::Illegal(_))
        ));
    }

    #[test]
    fn hybrid_pip_paid_by_either_color() {
        // {G/U} payable by green alone or by blue alone (CR 107.4d).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Hybrid(ColorPip::G, ColorPip::U)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.green = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.green, 0);

        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.blue = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.blue, 0);
    }

    #[test]
    fn hybrid_solve_avoids_greedy_dead_end() {
        // {G/U}{G} with one G + one U: the {G/U} must take U so the {G} can take G (CR 107.4).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Hybrid(ColorPip::G, ColorPip::U), ManaSymbol::G],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.green = 1;
        e.state.players[0].mana_pool.blue = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.green, 0);
        assert_eq!(e.state.players[0].mana_pool.blue, 0);
    }

    #[test]
    fn mono_hybrid_paid_by_generic_when_color_absent() {
        // {2/W} payable by two generic when no white is available (CR 107.4e).
        let cost = ManaCost {
            pips: vec![ManaSymbol::MonoHybrid(2, ColorPip::W)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 2;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.red, 0);

        // ...or by one white (the cheaper alternative is preferred, leaving generic mana spare).
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.white = 1;
        e.state.players[0].mana_pool.red = 2;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.white, 0);
        assert_eq!(e.state.players[0].mana_pool.red, 2);
    }

    #[test]
    fn mono_hybrid_rejected_with_one_generic() {
        // One spare mana can't cover the {2/...} generic alternative.
        let cost = ManaCost {
            pips: vec![ManaSymbol::MonoHybrid(2, ColorPip::W)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 1;
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 0, 0, &[]),
            Err(EngineError::Illegal(_))
        ));
    }

    #[test]
    fn phyrexian_paid_by_mana() {
        // {B/P} with no life flag must be paid by black mana (CR 107.4f).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Phyrexian(ColorPip::B)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.black = 1;
        let life = e.state.players[0].life;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.black, 0);
        assert_eq!(e.state.players[0].life, life); // no life paid
    }

    #[test]
    fn phyrexian_paid_by_life() {
        // {B/P} with pay_life on pip 0 deducts 2 life and touches no mana (CR 107.4f).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Phyrexian(ColorPip::B)],
        };
        let mut e = engine_with_priority();
        let life = e.state.players[0].life;
        let flex = [rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: true,
        }];
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &flex).is_ok());
        assert_eq!(e.state.players[0].life, life - 2);
    }

    #[test]
    fn phyrexian_life_rejected_when_insufficient_life() {
        // CR 119.4: can't pay 2 life with only 1 (and no mana to fall back on).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Phyrexian(ColorPip::B)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].life = 1;
        let flex = [rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: true,
        }];
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 0, 0, &flex),
            Err(EngineError::Illegal(_))
        ));
        assert_eq!(e.state.players[0].life, 1); // unchanged on failure
    }
}
