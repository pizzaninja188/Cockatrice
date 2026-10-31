use super::combat::priority_locked_for_combat_declaration;
use super::events::{ev_log, ev_priority_changed, finish_with_events, format_spell_targets_log};
use super::legal_actions::fill_legal;
use super::payment::PaidCardCost;
#[cfg(test)]
use super::payment::{commit_mana_payment, pay_mana, plan_mana_payment_with_reduction};
use super::presentation::{
    ability_presentation, activated_stack_mode_presentation, child_presentation_ref,
    spell_stack_presentation, PresentationPath,
};
use super::targeting::{
    capture_stack_target, face_mode_program, face_program, target_filter_legal_at_resolution,
    validate_ability_targets_with_context_and_program, validate_spell_targets_with_program,
    validate_x_target_mana_value, TargetResolutionContext, TargetSourceIdentity,
};
use super::*;
use crate::engine::events::ev_log_ability;
use prost::Message;
use tricerules_card_model::ActivationTiming;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LimitedActivationUse {
    PerTurn(ActivationUseKey),
    PerObject(PersistentActivationUseKey),
}

/// The permission selected at announcement, before paying costs can change the battlefield.
#[derive(Clone, Copy)]
enum LoyaltyActivationAuthorization {
    Ordinary,
    ChainVeil {
        permission_id: u64,
        walker: ObjectId,
        generation: u64,
    },
}

/// Announcement receipts captured before costs can remove sources, targets or grants.
#[derive(Clone)]
pub(super) struct AnnouncedAbilityActivation {
    player: PlayerId,
    permanent_id: ObjectId,
    source_owner: PlayerId,
    source_zone_change: u64,
    source_face_change: u64,
    source_linked_exile_occurrence: Option<LinkedExileOccurrence>,
    ability_index: usize,
    card_id: String,
    face_up_index: usize,
    ability: ActivatedAbilityDef,
    activated_ability_occurrence: ActivatedAbilityOccurrence,
    ability_text: String,
    source_token_identity: Option<rv1::TokenIdentity>,
    card_name: String,
    pub(super) activation_reveal_id: Option<String>,
    primary_presentation: Option<rv1::PresentationRef>,
    targets: Vec<rv1::TargetRef>,
    stack_targets: Vec<StackTarget>,
    chosen_modes: Vec<ChosenMode>,
    reserved_virtual_id: ObjectId,
    crime_events: Vec<GameEvent>,
    target_triggers: Vec<super::triggers::CollectedTrigger>,
    activation_uses: Vec<LimitedActivationUse>,
    loyalty_authorization: Option<LoyaltyActivationAuthorization>,
    /// The actor activated a planeswalker loyalty ability at announcement time, before costs
    /// can remove the source or alter its characteristics. Committed only after costs succeed.
    planeswalker_loyalty_activation: bool,
    x_value: u32,
}

impl AnnouncedAbilityActivation {
    pub(super) fn source_identity(&self) -> (PlayerId, ObjectId, u64) {
        (self.player, self.permanent_id, self.source_zone_change)
    }
}

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

fn mana_amount_for_color(
    color: tricerules_card_model::Color,
    count: u32,
) -> tricerules_card_model::ManaAmount {
    use tricerules_card_model::Color;
    match color {
        Color::White => tricerules_card_model::ManaAmount {
            w: count,
            ..Default::default()
        },
        Color::Blue => tricerules_card_model::ManaAmount {
            u: count,
            ..Default::default()
        },
        Color::Black => tricerules_card_model::ManaAmount {
            b: count,
            ..Default::default()
        },
        Color::Red => tricerules_card_model::ManaAmount {
            r: count,
            ..Default::default()
        },
        Color::Green => tricerules_card_model::ManaAmount {
            g: count,
            ..Default::default()
        },
    }
}

fn color_for_index(index: usize) -> tricerules_card_model::Color {
    use tricerules_card_model::Color;
    match index {
        0 => Color::White,
        1 => Color::Blue,
        2 => Color::Black,
        3 => Color::Red,
        4 => Color::Green,
        _ => unreachable!("five-color mask index"),
    }
}

fn mana_color_mask(amount: tricerules_card_model::ManaAmount) -> u8 {
    u8::from(amount.w > 0)
        | (u8::from(amount.u > 0) << 1)
        | (u8::from(amount.b > 0) << 2)
        | (u8::from(amount.r > 0) << 3)
        | (u8::from(amount.g > 0) << 4)
}

fn multiply_mana_amount(
    amount: tricerules_card_model::ManaAmount,
    multiplier: u32,
) -> tricerules_card_model::ManaAmount {
    tricerules_card_model::ManaAmount {
        w: amount.w.saturating_mul(multiplier),
        u: amount.u.saturating_mul(multiplier),
        b: amount.b.saturating_mul(multiplier),
        r: amount.r.saturating_mul(multiplier),
        g: amount.g.saturating_mul(multiplier),
        c: amount.c.saturating_mul(multiplier),
    }
}

/// CR 702.8b: true if the card face is castable at instant speed (is an instant, or has flash).
pub(super) fn castable_at_instant_speed(face: &tricerules_card_model::FaceRef<'_>) -> bool {
    face.is_instant
        || face
            .keywords
            .contains(&tricerules_card_model::Keyword::Flash)
}

fn command_satisfies_cast_cost_condition(
    selections: &[rv1::CastCostGroupSelection],
    groups: &[CastCostGroupDef],
    condition: &tricerules_card_model::CastCostReceiptCondition,
) -> bool {
    let selected = groups
        .iter()
        .position(|group| group.group_id == condition.group_id)
        .and_then(|group_index| {
            groups[group_index]
                .options
                .iter()
                .position(|option| option.option_id() == &condition.option_id)
                .map(|option_index| (group_index as u32, option_index as u32))
        })
        .is_some_and(|(group_index, option_index)| {
            selections.iter().any(|selection| {
                selection.group_index == group_index && selection.option_index == option_index
            })
        });
    selected == condition.expected_selected
}

fn validate_cast_cost_target_expansions(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    targeting: Option<&TargetingDef>,
    targets: &[rv1::TargetRef],
    selections: &[rv1::CastCostGroupSelection],
    groups: &[CastCostGroupDef],
) -> Result<(), EngineError> {
    let Some(targeting) = targeting else {
        return Ok(());
    };
    for (group_index, group) in targeting.groups.iter().enumerate() {
        let Some(expansion) = &group.cast_cost_expansion else {
            continue;
        };
        if command_satisfies_cast_cost_condition(selections, groups, &expansion.condition) {
            continue;
        }
        for target in targets
            .iter()
            .filter(|target| target.group_index == group_index as u32)
        {
            if rv1::TargetRefKind::try_from(target.kind) != Ok(rv1::TargetRefKind::Permanent)
                || !target_filter_legal_at_resolution(
                    engine,
                    &expansion.without_cost,
                    target.object_id,
                    caster,
                    source,
                    TriggerContext::default(),
                )
            {
                return Err(EngineError::Illegal(
                    "target requires its linked cast cost option",
                ));
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
pub(in crate::engine) struct PreparedSpellCast {
    idx: usize,
    pub oid: ObjectId,
    card_id: String,
    face_name: String,
    has_x: bool,
    chosen_x: u32,
    needs_face_annotation: bool,
    cast_method: SpellCastMethod,
    public_targets: Vec<rv1::TargetRef>,
    chosen_modes: Vec<ChosenMode>,
    chosen_mode_indices: Vec<u32>,
    chosen_mode_labels: Vec<String>,
    mana_value: u32,
    pub convoke: bool,
    pub payment: super::payment::transaction::PreparedPaymentCosts,
}

#[derive(Clone)]
pub(in crate::engine) struct PendingSpellCastInternal {
    pub(in crate::engine) prepared: PreparedSpellCast,
    original_object: GameObject,
    restored_permissions: Vec<(usize, ActiveExilePlayPermission)>,
    restored_discard_successors: Vec<((ObjectId, u64), u64)>,
    proposal_triggers: Vec<super::triggers::CollectedTrigger>,
    crime_events: Vec<GameEvent>,
}

fn cast_from_announcement(announcement: &rv1::SpellCastAnnouncement) -> rv1::CastSpell {
    rv1::CastSpell {
        targets: announcement.targets.clone(),
        x_value: announcement.x_value,
        flex_payments: announcement.flex_payments.clone(),
        face_index: announcement.face_index,
        selected_modes: announcement.selected_modes.clone(),
        source: announcement.source,
        cost_selections: announcement.cost_selections.clone(),
        restricted_mana: Vec::new(),
        cast_cost_group_selections: announcement.cast_cost_group_selections.clone(),
        cast_method: announcement.cast_method,
        payment: None,
        casting_permission_id: announcement.casting_permission_id,
        prototyped: announcement.prototyped,
    }
}

pub(in crate::engine) fn announcement_from_cast(
    cast: &rv1::CastSpell,
) -> rv1::SpellCastAnnouncement {
    rv1::SpellCastAnnouncement {
        targets: cast.targets.clone(),
        x_value: cast.x_value,
        flex_payments: cast.flex_payments.clone(),
        face_index: cast.face_index,
        selected_modes: cast.selected_modes.clone(),
        source: cast.source,
        cost_selections: cast.cost_selections.clone(),
        cast_cost_group_selections: cast.cast_cost_group_selections.clone(),
        cast_method: cast.cast_method,
        casting_permission_id: cast.casting_permission_id,
        prototyped: cast.prototyped,
    }
}

fn validate_modal_permanent_target_generations(
    engine: &GameEngine,
    targets: &[rv1::TargetRef],
) -> Result<(), EngineError> {
    for target in targets {
        let kind = rv1::TargetRefKind::try_from(target.kind).ok();
        let is_permanent = kind == Some(rv1::TargetRefKind::Permanent)
            || (kind == Some(rv1::TargetRefKind::Unspecified)
                && engine
                    .state
                    .objects
                    .get(&target.object_id)
                    .is_some_and(|object| object.zone == Zone::Battlefield));
        if !is_permanent {
            continue;
        }
        let expected = target
            .expected_zone_change_generation
            .ok_or(EngineError::Illegal(
                "modal permanent target generation is required",
            ))?;
        if engine
            .state
            .zone_change_generation
            .get(&target.object_id)
            .copied()
            .unwrap_or(0)
            != expected
        {
            return Err(EngineError::Illegal("target generation is stale"));
        }
    }
    Ok(())
}

impl GameEngine {
    /// CR 205.4e is checked when casting begins, before costs can remove a qualifier.
    pub(super) fn legendary_spell_cast_allowed(
        &self,
        player: PlayerId,
        face: &tricerules_card_model::CardFace,
    ) -> bool {
        if !face.is_legendary || !(face.is_instant || face.is_sorcery) {
            return true;
        }
        self.state.objects.values().any(|object| {
            object.zone == Zone::Battlefield
                && self
                    .characteristics(object.id)
                    .is_some_and(|characteristics| {
                        characteristics.controller == player
                            && characteristics.is_legendary()
                            && (characteristics.is_creature()
                                || characteristics.has_type("Planeswalker"))
                    })
        })
    }

    /// One resolver for activation legality, published labels and atomic payment planning.
    /// None denotes an undefined commander cost or overflow, never a zero-life payment.
    pub(super) fn activated_life_cost(
        &self,
        player: PlayerId,
        costs: &[AbilityCost],
    ) -> Option<u32> {
        let state = &self.state.players[self.state.player_idx(player)?];
        costs.iter().try_fold(0u32, |total, cost| {
            let amount = match cost {
                AbilityCost::PayLife { amount } => *amount,
                AbilityCost::PayCommanderColorIdentityLife => {
                    if !state.has_declared_commander {
                        return None;
                    }
                    u32::try_from(state.color_identity.len()).ok()?
                }
                _ => 0,
            };
            total.checked_add(amount)
        })
    }

    /// Mana abilities can create new payment resources after CR 601.2f locks the cost.
    /// Refresh eligibility against the accepted spell face without preparing its cost again.
    pub(super) fn eligible_restricted_mana_for_pending_spell_cast(
        &self,
        pending: &crate::state::PendingSpellCastState,
    ) -> Result<Vec<u32>, EngineError> {
        let prepared = &self
            .pending_spell_cast_internal
            .as_ref()
            .ok_or(EngineError::Illegal("missing locked spell cost"))?
            .prepared;
        let face = self
            .registry
            .get(&prepared.card_id)
            .and_then(|definition| definition.face(pending.announcement.face_index as usize))
            .ok_or(EngineError::Illegal("locked spell face is unavailable"))?;
        let face = if pending.announcement.prototyped {
            face.prototyped_face()
                .ok_or(EngineError::Illegal("locked spell has no Prototype mode"))?
        } else {
            (*face).clone()
        };
        Ok(self.eligible_restricted_mana_for_spell(prepared.idx, &face))
    }

    pub(super) fn begin_spell_cast(
        &mut self,
        player: PlayerId,
        command: &rv1::BeginSpellCast,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.pending_spell_cast.is_some() || self.pending_spell_cast_internal.is_some() {
            return Err(EngineError::Illegal("a spell cast is already being paid"));
        }
        let announcement = command
            .announcement
            .as_ref()
            .ok_or(EngineError::Illegal("missing spell announcement"))?;
        let special = self.special_cast_method(player).is_some();
        if !special
            && (self.state.priority_player_id() != player || self.state.blocking_choice().is_some())
        {
            return Err(EngineError::Illegal(
                "spell announcement is not available now",
            ));
        }
        let cast = cast_from_announcement(announcement);
        let prepared = self.prepare_spell_cast(player, &cast)?;
        let oid = prepared.oid;
        let object = self
            .state
            .objects
            .get(&oid)
            .cloned()
            .ok_or(EngineError::Illegal("spell source missing"))?;
        let original_zone = object.zone;
        let original_holder = self
            .state
            .players
            .iter()
            .find(|candidate| match original_zone {
                Zone::Hand => candidate.hand.contains(&oid),
                Zone::Graveyard => candidate.graveyard.contains(&oid),
                Zone::Exile => candidate.exile.contains(&oid),
                Zone::Library => candidate.library.contains(&oid),
                _ => false,
            })
            .map(|candidate| candidate.id)
            .ok_or(EngineError::Illegal(
                "spell source is not in a castable zone",
            ))?;
        let holder_idx = self
            .state
            .player_idx(original_holder)
            .ok_or(EngineError::UnknownPlayer(original_holder))?;
        let original_position = match original_zone {
            Zone::Hand => self.state.players[holder_idx]
                .hand
                .iter()
                .position(|id| *id == oid),
            Zone::Graveyard => self.state.players[holder_idx]
                .graveyard
                .iter()
                .position(|id| *id == oid),
            Zone::Exile => self.state.players[holder_idx]
                .exile
                .iter()
                .position(|id| *id == oid),
            Zone::Library => self.state.players[holder_idx]
                .library
                .iter()
                .position(|id| *id == oid),
            _ => None,
        }
        .ok_or(EngineError::Illegal("spell source position is unavailable"))?;
        let original_generation_entry = self.state.zone_change_generation.get(&oid).copied();
        let original_generation = original_generation_entry.unwrap_or(0);
        let stack_generation = original_generation.saturating_add(1);
        let public_targets = prepared.public_targets.clone();
        let stack_targets = public_targets
            .iter()
            .map(|target| capture_stack_target(self, target))
            .collect();
        let mut proposal_triggers = self.collect_event_triggers(&[GameEvent::TargetsChosen {
            controller: player,
            source: TargetingSourceKind::SpellCast,
            stack_object: StackObjectRef {
                object_id: oid,
                zone_change_generation: Some(stack_generation),
            },
            targets: stack_targets,
        }]);
        let crime_events = self
            .crime_event(player, &public_targets)
            .into_iter()
            .collect();
        let cast_departure = (original_zone == Zone::Graveyard).then(|| self.snapshot_zone_event());
        let restored_permissions = self
            .state
            .active_exile_play_permissions
            .iter()
            .cloned()
            .enumerate()
            .filter(|(_, permission)| permission.object_id == oid)
            .collect();
        let restored_discard_successors = self
            .state
            .discard_reference_successors
            .iter()
            .filter(|((object_id, _), _)| *object_id == oid)
            .map(|(key, value)| (*key, *value))
            .collect();
        let locked_total_cost = prepared.payment.total_cost_label()?;
        // Capture the public identity receipt before reserving the card on the Stack. A face-down
        // exiled card turns face up just before its accepted cast announcement (CR 406.3a / 601.2a);
        // cancellation restores the original object snapshot but does not erase public knowledge.
        let face_down_exile_reveal_events = super::reveals::face_down_exile_reveal_events(
            &self.state,
            self.registry,
            oid,
            "casting a card from exile",
        )
        .unwrap_or_default();
        let reveals_face_down_exile = !face_down_exile_reveal_events.is_empty();
        let face_index = prepared.idx;

        // Everything fallible above is validation. This is the CR 601.2a reservation edge.
        super::resolution::move_object_to_zone(
            &mut self.state,
            self.registry,
            oid,
            Zone::Stack,
            None,
        )?;
        if reveals_face_down_exile {
            let object = self
                .state
                .objects
                .get_mut(&oid)
                .expect("the accepted cast retains its reserved physical object");
            debug_assert_eq!(object.zone, Zone::Stack);
            object.face_down = false;
            object.face_up_index = face_index;
        }
        if let Some(snapshot) = cast_departure {
            let event = self.finish_single_zone_event(snapshot, oid);
            proposal_triggers.extend(self.collect_event_triggers(&[event]));
        }
        let transaction_id = self.state.next_spell_cast_transaction_id;
        self.state.next_spell_cast_transaction_id = transaction_id.saturating_add(1);
        self.state.pending_spell_cast = Some(crate::state::PendingSpellCastState {
            transaction_id,
            caster: player,
            reserved_object_id: oid,
            announcement: announcement.clone(),
            locked_total_cost,
            original_zone,
            original_holder,
            original_position,
            original_zone_change_generation: original_generation,
            original_zone_change_generation_present: original_generation_entry.is_some(),
            resolution_time_offer: special,
        });
        self.pending_spell_cast_internal = Some(PendingSpellCastInternal {
            prepared,
            original_object: object,
            restored_permissions,
            restored_discard_successors,
            proposal_triggers,
            crime_events,
        });
        let mut batch = RuledEventBatch::default();
        batch.events.extend(face_down_exile_reveal_events);
        Ok(batch)
    }

    pub(super) fn commit_spell_cast(
        &mut self,
        player: PlayerId,
        command: &rv1::CommitSpellCast,
    ) -> Result<RuledEventBatch, EngineError> {
        let state = self
            .state
            .pending_spell_cast
            .as_ref()
            .ok_or(EngineError::Illegal("no spell cast is being paid"))?;
        if state.caster != player {
            return Err(EngineError::Illegal(
                "only the caster may commit this spell",
            ));
        }
        if state.transaction_id != command.transaction_id {
            return Err(EngineError::Illegal("stale spell-cast transaction"));
        }
        let current_generation = self
            .state
            .zone_change_generation
            .get(&state.reserved_object_id)
            .copied()
            .unwrap_or(0);
        if self
            .state
            .objects
            .get(&state.reserved_object_id)
            .is_none_or(|object| object.zone != Zone::Stack)
            || current_generation != state.original_zone_change_generation.saturating_add(1)
        {
            return Err(EngineError::Illegal("reserved spell source changed"));
        }
        let internal = self
            .pending_spell_cast_internal
            .clone()
            .ok_or(EngineError::Illegal("missing locked spell cost"))?;
        let mut prepared = internal.prepared.clone();
        prepared.payment.eligible_restricted_mana =
            self.eligible_restricted_mana_for_pending_spell_cast(state)?;
        prepared.payment.restricted_mana = command.restricted_mana.clone();
        let mut cast = cast_from_announcement(&state.announcement);
        cast.payment = command.payment.clone();
        cast.restricted_mana = command.restricted_mana.clone();
        let special = state.resolution_time_offer;
        let special_continuation = if special {
            let pending = self
                .state
                .pending_resolution
                .as_ref()
                .ok_or(EngineError::Illegal("special cast resolution disappeared"))?;
            let ResolutionContinuation::SpecialCast {
                stack,
                return_source_to_owner_hand_if_cast,
                completion,
                ..
            } = &pending.continuation
            else {
                return Err(EngineError::Illegal("special cast resolution changed"));
            };
            Some((
                stack.item.clone(),
                stack.resume_effect_index,
                stack.previous_result.clone(),
                *return_source_to_owner_hand_if_cast,
                completion.clone(),
            ))
        } else {
            None
        };
        let mut batch = self.commit_prepared_spell_cast(player, &cast, prepared, &internal)?;
        self.state.undoable_mana_abilities.clear();
        self.state.pending_spell_cast = None;
        self.pending_spell_cast_internal = None;
        if let Some((item, resume_effect_index, previous_result, return_source, completion)) =
            special_continuation
        {
            self.state.pending_resolution = None;
            let linked_cohort = self
                .state
                .pending_linked_exile_play
                .as_mut()
                .filter(|cohort| {
                    cohort.stack.item.id == item.id
                        && cohort.stack.item.source_zone_change == item.source_zone_change
                });
            if let Some(cohort) = linked_cohort {
                cohort.any_cast_committed = true;
            } else if return_source {
                self.return_linked_cast_source_to_owner_hand(&item, &mut batch.events)?;
            }
            let mut parked = ParkedStackResolution::new(item.clone())
                .with_previous_result(previous_result.clone());
            parked.resume_effect_index = resume_effect_index;
            let commander_choice =
                self.finish_special_cast_completion(&completion, true, parked, &mut batch.events)?;
            batch = if commander_choice {
                finish_with_events(self, batch.events)
            } else {
                self.complete_parked_resolution_with_previous(
                    item,
                    resume_effect_index,
                    previous_result,
                    batch.events,
                )?
            };
        }
        Ok(batch)
    }

    pub(in crate::engine) fn return_linked_cast_source_to_owner_hand(
        &mut self,
        item: &StackItem,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(source_id) = item.source_permanent_id else {
            return Ok(());
        };
        let current_generation = self
            .state
            .zone_change_generation
            .get(&source_id)
            .copied()
            .unwrap_or(0);
        let Some(source) = self.state.objects.get(&source_id) else {
            return Ok(());
        };
        if source.zone != Zone::Battlefield || current_generation != item.source_zone_change {
            return Ok(());
        }
        let owner = source.owner;
        let source_name = self
            .registry
            .get(&source.card_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| "the source permanent".to_string());
        let zone_snapshot = self.snapshot_zone_event();
        let leave_event = self.battlefield_leave_event(source_id);
        crate::engine::resolution::move_object_to_zone(
            &mut self.state,
            self.registry,
            source_id,
            Zone::Hand,
            None,
        )?;
        events.push(crate::engine::resolution::permanent_moved_event(
            &self.state,
            source_id,
            owner,
            rv1::permanent_moved::Destination::Hand,
        ));
        events.push(ev_log(format!(
            "{source_name} returns to its owner's hand after the linked card is cast."
        )));
        self.fire_zone_triggers(zone_snapshot, leave_event.into_iter().collect(), events);
        Ok(())
    }

    pub(super) fn cancel_spell_cast(
        &mut self,
        player: PlayerId,
        command: &rv1::CancelSpellCast,
    ) -> Result<RuledEventBatch, EngineError> {
        let state = self
            .state
            .pending_spell_cast
            .as_ref()
            .ok_or(EngineError::Illegal("no spell cast is being paid"))?;
        if state.caster != player {
            return Err(EngineError::Illegal(
                "only the caster may cancel this spell",
            ));
        }
        if state.transaction_id != command.transaction_id {
            return Err(EngineError::Illegal("stale spell-cast transaction"));
        }
        let state = self.state.pending_spell_cast.take().unwrap();
        let internal = self
            .pending_spell_cast_internal
            .take()
            .ok_or(EngineError::Illegal("missing reserved spell state"))?;
        self.restore_spell_cast_reservation(&state, internal)?;
        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!(
            "P{player} cancels the spell-casting proposal."
        )));
        if state.resolution_time_offer {
            if let Some(event) = self.resolution_payment_choice_event() {
                batch.events.push(event);
            }
        }
        Ok(batch)
    }

    fn restore_spell_cast_reservation(
        &mut self,
        state: &crate::state::PendingSpellCastState,
        internal: PendingSpellCastInternal,
    ) -> Result<(), EngineError> {
        let oid = state.reserved_object_id;
        for participant in &mut self.state.players {
            participant.hand.retain(|candidate| *candidate != oid);
            participant.graveyard.retain(|candidate| *candidate != oid);
            participant.exile.retain(|candidate| *candidate != oid);
            participant.library.retain(|candidate| *candidate != oid);
        }
        let holder_idx = self
            .state
            .player_idx(state.original_holder)
            .ok_or(EngineError::UnknownPlayer(state.original_holder))?;
        match state.original_zone {
            Zone::Hand => {
                let position = state
                    .original_position
                    .min(self.state.players[holder_idx].hand.len());
                self.state.players[holder_idx].hand.insert(position, oid);
            }
            Zone::Graveyard => {
                let position = state
                    .original_position
                    .min(self.state.players[holder_idx].graveyard.len());
                self.state.players[holder_idx]
                    .graveyard
                    .insert(position, oid);
            }
            Zone::Exile => {
                let position = state
                    .original_position
                    .min(self.state.players[holder_idx].exile.len());
                self.state.players[holder_idx].exile.insert(position, oid);
            }
            Zone::Library => {
                let position = state
                    .original_position
                    .min(self.state.players[holder_idx].library.len());
                self.state.players[holder_idx].library.insert(position, oid);
            }
            _ => {
                return Err(EngineError::Illegal(
                    "invalid spell origin for cancellation",
                ));
            }
        }
        self.state.objects.insert(oid, internal.original_object);
        if state.original_zone_change_generation_present {
            self.state
                .zone_change_generation
                .insert(oid, state.original_zone_change_generation);
        } else {
            self.state.zone_change_generation.remove(&oid);
        }
        self.state
            .active_exile_play_permissions
            .retain(|permission| permission.object_id != oid);
        for (position, permission) in internal.restored_permissions {
            let position = position.min(self.state.active_exile_play_permissions.len());
            self.state
                .active_exile_play_permissions
                .insert(position, permission);
        }
        self.state
            .discard_reference_successors
            .retain(|(object_id, _), _| *object_id != oid);
        for (key, value) in internal.restored_discard_successors {
            self.state.discard_reference_successors.insert(key, value);
        }
        Ok(())
    }

    /// Abandon an uncommitted spell proposal when either the caster or the card's owner leaves.
    /// Restore the exact original incarnation only while its owner remains in the game; keeping
    /// the reservation alive would block every surviving player, while restoring a departed
    /// owner's snapshot would recreate a card that CR 800.4a removed.
    pub(super) fn reconcile_spell_cast_departure(
        &mut self,
        departed: &HashSet<PlayerId>,
    ) -> Result<bool, EngineError> {
        let Some(pending) = self.state.pending_spell_cast.as_ref() else {
            return Ok(false);
        };
        if !departed.contains(&pending.caster) && !departed.contains(&pending.original_holder) {
            return Ok(false);
        }

        let state = self
            .state
            .pending_spell_cast
            .take()
            .expect("pending spell cast was checked above");
        let internal = self
            .pending_spell_cast_internal
            .take()
            .ok_or(EngineError::Illegal("missing reserved spell state"))?;
        if departed.contains(&state.original_holder)
            || departed.contains(&internal.original_object.owner)
        {
            let object_id = state.reserved_object_id;
            for participant in &mut self.state.players {
                participant.hand.retain(|candidate| *candidate != object_id);
                participant
                    .graveyard
                    .retain(|candidate| *candidate != object_id);
                participant
                    .exile
                    .retain(|candidate| *candidate != object_id);
            }
            self.state.objects.remove(&object_id);
            self.state.zone_change_generation.remove(&object_id);
            self.state
                .active_exile_play_permissions
                .retain(|permission| permission.object_id != object_id);
            self.state
                .discard_reference_successors
                .retain(|(candidate, _), _| *candidate != object_id);
            // `internal` deliberately drops the saved original object and proposal-only triggers.
        } else {
            self.restore_spell_cast_reservation(&state, internal)?;
        }
        Ok(true)
    }

    /// CR 601.2i / 608.2i: freeze automatic public conditions after all costs and cast facts
    /// have committed. Unlike cost receipts these facts do not transfer to an uncast copy.
    fn snapshot_completed_cast(&mut self, object_id: ObjectId) {
        let index = self
            .state
            .stack
            .iter()
            .position(|item| item.id == object_id)
            .expect("a completed cast has a stack item");
        let item = &self.state.stack[index];
        let context = ConditionContext {
            source_zone_change: self
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
            resolving_spell_id: None,
            ..ConditionContext::for_stack_item(item)
        };
        let results = self
            .registry
            .get(&item.card_id)
            .and_then(|definition| definition.face(item.face_index))
            .into_iter()
            .flat_map(|face| &face.cast_conditions)
            .map(|condition| self.condition_holds(condition, context))
            .collect();
        self.state.stack[index].cast_condition_results = results;
    }

    pub(in crate::engine) fn prepare_spell_cast(
        &self,
        player: PlayerId,
        command: &rv1::CastSpell,
    ) -> Result<PreparedSpellCast, EngineError> {
        let special_permission = self.special_cast_permission(player);
        let special_method = self.special_cast_method(player);
        let special = special_permission.is_some();
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
            Ok(rv1::CastMethod::Warp) => SpellCastMethod::Warp,
            Ok(rv1::CastMethod::Permission) => SpellCastMethod::Permission,
            Ok(rv1::CastMethod::Sneak) => SpellCastMethod::Sneak,
            Ok(rv1::CastMethod::SiegeDefeat) => SpellCastMethod::SiegeDefeat,
            Ok(rv1::CastMethod::Madness) => SpellCastMethod::Madness,
            Ok(rv1::CastMethod::Unspecified) | Err(_) => {
                return Err(EngineError::Illegal("missing or invalid cast method"));
            }
        };
        if self.state.turn_step == TurnStep::Cleanup && !special {
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
        if special
            && (special_method != Some(cast_method)
                || !matches!(source, rv1::cast_source::Location::ExileObjectId(_)))
        {
            return Err(EngineError::Illegal(
                "cast does not match active resolution offer",
            ));
        }
        let from_hand = matches!(source, rv1::cast_source::Location::HandIndex(_));
        let (oid, exile_permission_scope) = match source {
            rv1::cast_source::Location::HandIndex(hand_index) => {
                if command.casting_permission_id.is_some() {
                    return Err(EngineError::Illegal(
                        "hand casts cannot name an exile permission",
                    ));
                }
                if !matches!(
                    cast_method,
                    SpellCastMethod::Normal | SpellCastMethod::Warp | SpellCastMethod::Sneak
                ) {
                    return Err(EngineError::Illegal("invalid hand cast method"));
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
                if command.casting_permission_id.is_some() {
                    return Err(EngineError::Illegal(
                        "graveyard casts cannot name an exile permission",
                    ));
                }
                if !self.state.players[idx].graveyard.contains(source_oid) {
                    return Err(EngineError::Illegal("card is not in your graveyard"));
                }
                if !matches!(
                    cast_method,
                    SpellCastMethod::Flashback | SpellCastMethod::Harmonize
                ) {
                    return Err(EngineError::Illegal(
                        "graveyard casts require an alternative cast method",
                    ));
                }
                (*source_oid, None)
            }
            rv1::cast_source::Location::ExileObjectId(source_oid) => {
                let permission_id = command
                    .casting_permission_id
                    .ok_or(EngineError::Illegal("missing exile casting permission id"))?;
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
                    .filter(|_| !special)
                    .chain(special_permission.iter())
                    .find(|permission| {
                        permission.group_id == permission_id
                            && permission.object_id == *source_oid
                            && self.exile_permission_controller(permission) == Some(player)
                            && permission.zone_change_generation == generation
                            && permission.available_on_turn(self.state.turn_instance)
                            && match permission.scope {
                                ExilePlayPermissionScope::CastFace(expected) => {
                                    expected == face_index
                                }
                                ExilePlayPermissionScope::PlayCard
                                | ExilePlayPermissionScope::CastCard => true,
                            }
                    })
                    .ok_or(EngineError::Illegal(
                        "card has no cast-from-exile permission",
                    ))?;
                if permission.max_face_mana_value.is_some_and(|maximum| {
                    self.registry
                        .get(&object.card_id)
                        .and_then(|definition| definition.face(face_index))
                        .and_then(|face| {
                            if command.prototyped {
                                face.prototyped_face()
                            } else {
                                Some((*face).clone())
                            }
                        })
                        .is_none_or(|face| face.mana_cost.mana_value() > maximum)
                }) {
                    return Err(EngineError::Illegal(
                        "spell face exceeds the cast permission's mana-value limit",
                    ));
                }
                let method_matches = special_method == Some(cast_method)
                    || matches!(
                        (&permission.cast_cost, cast_method),
                        (
                            crate::state::ExilePermissionCastCost::PrintedManaCost,
                            SpellCastMethod::Normal
                        ) | (
                            crate::state::ExilePermissionCastCost::AlternativeManaCost(_),
                            SpellCastMethod::Permission
                        )
                    );
                if !method_matches {
                    return Err(EngineError::Illegal(
                        "cast method does not match exile permission cost",
                    ));
                }
                if object.zone != Zone::Exile {
                    return Err(EngineError::Illegal("illegal cast from exile"));
                }
                (
                    *source_oid,
                    Some((permission.scope, permission.cast_cost.clone())),
                )
            }
            rv1::cast_source::Location::LibraryObjectId(source_oid) => {
                if special
                    || command.casting_permission_id.is_some()
                    || cast_method != SpellCastMethod::Normal
                {
                    return Err(EngineError::Illegal(
                        "library-top casts use a normal cast without an exile permission id",
                    ));
                }
                let object = self
                    .state
                    .objects
                    .get(source_oid)
                    .ok_or(EngineError::Illegal("unknown library object"))?;
                if object.zone != Zone::Library
                    || object.owner != player
                    || self.state.players[idx].library.front() != Some(source_oid)
                {
                    return Err(EngineError::Illegal(
                        "library cast source is not the current top card",
                    ));
                }
                let definition = self
                    .registry
                    .get(&object.card_id)
                    .ok_or_else(|| EngineError::MissingCard(object.card_id.clone()))?;
                let face = definition
                    .face(face_index)
                    .ok_or(EngineError::Illegal("bad face index"))?;
                if face.is_land
                    || !definition.face_available_from_hand(face_index)
                    || !self.may_cast_library_top_face(player, *source_oid, definition, face_index)
                {
                    return Err(EngineError::Illegal(
                        "selected face has no active cast-from-library permission",
                    ));
                }
                (*source_oid, None)
            }
        };
        if !from_hand
            && command
                .source
                .as_ref()
                .and_then(|source| source.expected_zone_change_generation)
                != Some(
                    self.state
                        .zone_change_generation
                        .get(&oid)
                        .copied()
                        .unwrap_or(0),
                )
        {
            return Err(EngineError::Illegal(
                "missing or stale public cast generation",
            ));
        }
        let target_source = TargetSourceIdentity::spell_face(self, oid, face_index, player);
        let card_id = self.state.objects.get(&oid).unwrap().card_id.clone();
        let def = self
            .registry
            .get(&card_id)
            .ok_or_else(|| EngineError::MissingCard(card_id.clone()))?;
        let face = def
            .face(face_index)
            .ok_or(EngineError::Illegal("bad face index"))?;
        let characteristic_face = if command.prototyped {
            face.prototyped_face()
                .ok_or(EngineError::Illegal("card has no Prototype mode"))?
        } else {
            (*face).clone()
        };
        if !self.legendary_spell_cast_allowed(player, face) {
            return Err(EngineError::Illegal(
                "legendary spell requires a legendary creature or planeswalker",
            ));
        }
        let needs_face_annotation =
            super::legal_actions::cast_face_count_for_source(self, player, source) > 1
                || def
                    .face_display_name(face_index)
                    .is_some_and(|display_name| display_name != face.name.as_str());
        if from_hand && !def.face_available_from_hand(face_index) {
            return Err(EngineError::Illegal("face cannot be cast from hand"));
        }
        if matches!(
            exile_permission_scope
                .as_ref()
                .map(|permission| permission.0),
            Some(ExilePlayPermissionScope::CastCard | ExilePlayPermissionScope::PlayCard)
        ) && !def.face_available_from_hand(face_index)
        {
            return Err(EngineError::Illegal(
                "face cannot be cast with this permission",
            ));
        }
        if matches!(
            exile_permission_scope,
            Some((ExilePlayPermissionScope::CastFace(_), _))
        ) && !special
            && !self.state.prepare_spell_sources.contains_key(&oid)
            && !face.is_permanent()
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
            SpellCastMethod::Normal => characteristic_face.mana_cost.clone(),
            SpellCastMethod::Warp => face
                .warp_cost
                .clone()
                .ok_or(EngineError::Illegal("card has no Warp cost"))?,
            SpellCastMethod::Sneak => face
                .sneak_cost
                .clone()
                .ok_or(EngineError::Illegal("card has no Sneak cost"))?,
            SpellCastMethod::Flashback => face
                .flashback_cost
                .clone()
                .ok_or(EngineError::Illegal("card has no flashback cost"))?,
            SpellCastMethod::Harmonize => face
                .harmonize_cost
                .clone()
                .ok_or(EngineError::Illegal("card has no harmonize cost"))?,
            SpellCastMethod::Permission => match exile_permission_scope.as_ref() {
                Some((_, crate::state::ExilePermissionCastCost::AlternativeManaCost(cost))) => {
                    cost.clone()
                }
                _ => {
                    return Err(EngineError::Illegal(
                        "permission cast has no alternative mana cost",
                    ));
                }
            },
            SpellCastMethod::SiegeDefeat | SpellCastMethod::Madness => {
                match exile_permission_scope.as_ref() {
                    Some((_, crate::state::ExilePermissionCastCost::AlternativeManaCost(cost)))
                        if special_method == Some(cast_method) =>
                    {
                        cost.clone()
                    }
                    _ => {
                        return Err(EngineError::Illegal(
                            "special cast requires active resolution offer",
                        ));
                    }
                }
            }
        };
        let face_name = face.name.to_string();
        let face_effects: Vec<SpellEffectKind> = face.spell_effect.to_vec();
        let face_targeting = face.targeting.clone();
        let modal_spell = face.modal_spell.clone();
        let additional_costs = face.additional_costs.clone();
        let cast_cost_groups = face.cast_cost_groups.clone();
        let cost_modifiers = face.cost_modifiers.clone();
        let eligible_restricted_mana =
            self.eligible_restricted_mana_for_spell(idx, &characteristic_face);
        let sorcery_ok = super::priority::sorcery_speed_available(&self.state, player);
        let instant_ok = super::priority::instant_timing_step_allowed(&self.state);
        let conditional_instant = face
            .instant_speed_cast_cost
            .as_ref()
            .is_some_and(|condition| {
                command_satisfies_cast_cost_condition(
                    cast_cost_group_selections,
                    &face.cast_cost_groups,
                    condition,
                )
            });
        if special {
            // The resolving effect supplies timing permission (CR 608.2g).
        } else if cast_method == SpellCastMethod::Sneak {
            if !instant_ok || self.state.active_player_id() != player {
                return Err(EngineError::Illegal("Sneak is not available now"));
            }
        } else if face_is_sorcery {
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
        match self.state.blocking_choice().filter(|_| !special) {
            Some(BlockingChoice::AbilityActivation) => {
                return Err(EngineError::Illegal(
                    "must finish ability activation before casting",
                ));
            }
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
            Some(BlockingChoice::AttackDeclaration) => {
                return Err(EngineError::Illegal(
                    "must pay or cancel the pending attack declaration before casting",
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
            let selected_mode_indices = selected_modes
                .iter()
                .map(|selection| selection.mode_index as usize)
                .collect::<HashSet<_>>();
            if let Some(link) = &modal.all_modes_cast_cost {
                let condition = CastCostReceiptCondition {
                    group_id: link.group_id.clone(),
                    option_id: link.option_id.clone(),
                    expected_selected: true,
                };
                let submitted = command_satisfies_cast_cost_condition(
                    cast_cost_group_selections,
                    &cast_cost_groups,
                    &condition,
                );
                if submitted != (selected_mode_indices.len() == modal.modes.len()) {
                    return Err(EngineError::Illegal(
                        "selecting every mode and the all-modes cast cost must match exactly",
                    ));
                }
            }
            for (mode_index, mode) in modal.modes.iter().enumerate() {
                let Some(link) = &mode.linked_cast_cost else {
                    continue;
                };
                let group_index = cast_cost_groups
                    .iter()
                    .position(|group| group.group_id == link.group_id)
                    .ok_or(EngineError::Illegal(
                        "mode links an unknown cast cost group",
                    ))?;
                let option_index = cast_cost_groups[group_index]
                    .options
                    .iter()
                    .position(|option| option.option_id() == &link.option_id)
                    .ok_or(EngineError::Illegal(
                        "mode links an unknown cast cost option",
                    ))?;
                let submitted = cast_cost_group_selections.iter().any(|selection| {
                    selection.group_index == group_index as u32
                        && selection.option_index == option_index as u32
                });
                if submitted != selected_mode_indices.contains(&mode_index) {
                    return Err(EngineError::Illegal(
                        "selected modes and linked cast costs must match exactly",
                    ));
                }
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
                validate_spell_targets_with_program(
                    self,
                    player,
                    target_source,
                    &mode.effects,
                    mode.targeting.as_ref(),
                    &selection.targets,
                    face_mode_program(self, &card_id, &face.face_id, &mode.mode_id),
                )?;
                validate_cast_cost_target_expansions(
                    self,
                    player,
                    target_source,
                    mode.targeting.as_ref(),
                    &selection.targets,
                    cast_cost_group_selections,
                    &cast_cost_groups,
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
                chosen_mode_labels.push(mode_fallback(&face_name, &mode.mode_id));
                chosen_modes.push(ChosenMode {
                    mode_id: mode.mode_id.clone(),
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
            validate_spell_targets_with_program(
                self,
                player,
                target_source,
                &face_effects,
                face_targeting.as_ref(),
                targets,
                face_program(self, &card_id, &face.face_id),
            )?;
            validate_cast_cost_target_expansions(
                self,
                player,
                target_source,
                face_targeting.as_ref(),
                targets,
                cast_cost_group_selections,
                &cast_cost_groups,
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

        let generic_cost_reduction = self
            .spell_generic_reduction(
                player,
                oid,
                &characteristic_face,
                &characteristic_face.cost_modifiers,
            )
            .saturating_add(self.spell_target_generic_reduction(
                player,
                target_source,
                &public_targets,
                &cost_modifiers,
            ));
        let payment_plan = self.prepare_spell_costs(
            player,
            idx,
            oid,
            &face_mana,
            chosen_x,
            extra_generic,
            generic_cost_reduction,
            flex_payments,
            &additional_costs,
            cost_selections,
            &cast_cost_groups,
            cast_cost_group_selections,
            restricted_mana,
            &eligible_restricted_mana,
            cast_method,
            Some(&crate::state::CastCostAbilityOrigin {
                original_card_id: card_id.clone(),
                face_id: face.face_id.clone(),
                copy_revision: self.state.objects[&oid].copy_revision,
            }),
        )?;

        Ok(PreparedSpellCast {
            idx,
            oid,
            card_id,
            face_name,
            has_x,
            chosen_x,
            needs_face_annotation,
            cast_method,
            public_targets,
            chosen_modes,
            chosen_mode_indices,
            chosen_mode_labels,
            mana_value: characteristic_face.mana_cost.mana_value_on_stack(x_value),
            convoke: face.keywords.contains(&Keyword::Convoke),
            payment: payment_plan,
        })
    }

    pub(super) fn cast_spell(
        &mut self,
        player: PlayerId,
        command: &rv1::CastSpell,
    ) -> Result<RuledEventBatch, EngineError> {
        let mut begin_batch = self.begin_spell_cast(
            player,
            &rv1::BeginSpellCast {
                announcement: Some(announcement_from_cast(command)),
            },
        )?;
        let pending = self
            .state
            .pending_spell_cast
            .as_ref()
            .expect("an accepted spell announcement creates a transaction");
        let transaction_id = pending.transaction_id;
        let reserved_object_id = pending.reserved_object_id;
        let mut payment = command.payment.clone();
        if let Some(selection) = payment.as_mut() {
            // Legacy fixtures previewed before the source reservation. The production client
            // previews only after BeginSpellCast, where the same physical card has its stack
            // generation. Retarget this compatibility selection to that reserved occurrence.
            selection.source = Some(self.payment_object_ref(reserved_object_id));
        }
        let commit = rv1::CommitSpellCast {
            transaction_id,
            payment,
            restricted_mana: command.restricted_mana.clone(),
        };
        match self.commit_spell_cast(player, &commit) {
            Ok(batch) => {
                // This compatibility command combines Begin and Commit into one response. Keep
                // the accepted announcement's public reveal receipt before stack publication.
                begin_batch.events.extend(batch.events);
                Ok(begin_batch)
            }
            Err(error) => {
                // The obsolete atomic fixture shape cannot leave an interactive transaction open.
                // Roll its proposal back so rejected legacy scenario commands retain the same
                // all-or-nothing contract while still exercising the production lifecycle.
                let _ = self.cancel_spell_cast(player, &rv1::CancelSpellCast { transaction_id });
                self.state.next_spell_cast_transaction_id = transaction_id;
                Err(error)
            }
        }
    }

    fn commit_prepared_spell_cast(
        &mut self,
        player: PlayerId,
        command: &rv1::CastSpell,
        prepared: PreparedSpellCast,
        pending: &PendingSpellCastInternal,
    ) -> Result<RuledEventBatch, EngineError> {
        let PreparedSpellCast {
            idx,
            oid,
            card_id,
            face_name,
            has_x,
            chosen_x,
            needs_face_annotation,
            cast_method,
            public_targets,
            chosen_modes,
            chosen_mode_indices,
            chosen_mode_labels,
            mana_value,
            payment,
            convoke,
        } = prepared;
        let face_index = command.face_index as usize;
        let payment_plan = if let Some(selection) = &command.payment {
            let life = self.validate_explicit_payment(player, oid, convoke, &payment, selection)?;
            payment.finish_explicit_with_any_color(
                &self.state,
                selection,
                life,
                self.may_spend_mana_as_any_color(player),
            )?
        } else {
            payment.finish_with_any_color(&self.state, self.may_spend_mana_as_any_color(player))?
        };

        let trefs: Vec<ObjectId> = public_targets
            .iter()
            .map(|target| target.object_id)
            .collect();
        let stack_targets: Vec<_> = public_targets
            .iter()
            .map(|target| capture_stack_target(self, target))
            .collect();
        let pending_state = self
            .state
            .pending_spell_cast
            .as_ref()
            .ok_or(EngineError::Illegal("missing pending cast state"))?;
        let mut target_triggers = pending.proposal_triggers.clone();
        let crime_events = pending.crime_events.clone();
        let origin = pending_state.original_zone;
        let prepare_source = self.state.prepare_spell_sources.get(&oid).cloned();
        let payment_plan = self.prepare_cost_transaction_commit(payment_plan)?;
        let payment = self.commit_prevalidated_cost_transaction(payment_plan, None)?;
        let returned_attacker_assignment = payment.returned_attacker_assignment;
        let life_paid = payment.life_paid;
        let mana_spent = payment.mana_spent;
        let mana_spending_keywords =
            self.mana_spending_keywords_for_spell(&card_id, face_index, &command.restricted_mana);
        let mut paid_costs_line = format_paid_card_costs_log(&payment.paid_card_costs);
        if let Some(returned_name) = &payment.sneak_returned_name {
            let returned = format!("returning {returned_name}");
            paid_costs_line = if paid_costs_line.is_empty() {
                format!(" {returned}")
            } else {
                format!("{paid_costs_line} and {returned}")
            };
        }
        let payment_result = CardResultCohort {
            cards: payment
                .paid_card_costs
                .iter()
                .map(|cost| cost.result().clone())
                .collect(),
            program_discard_result: None,
        };
        let cast_cost_receipts = payment.cast_cost_receipts;
        let chosen_cast_cost_labels = cast_cost_receipts
            .iter()
            .map(|receipt| receipt.label.clone())
            .collect::<Vec<_>>();

        let tgt_line = format_spell_targets_log(&self.state, self.registry, &trefs);

        let stack_presentation = spell_stack_presentation(
            self.registry,
            &card_id,
            face_index,
            &chosen_modes,
            &cast_cost_receipts,
        );
        self.state
            .stack_presentations
            .insert(oid, stack_presentation.clone());
        if !mana_spending_keywords.is_empty() {
            self.state.spell_effects_carry_to_permanent.insert(oid);
        }
        for keyword in mana_spending_keywords {
            self.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::Layer6AddKeyword(keyword),
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: self.state.command_index,
            });
        }
        if let Some(object) = self.state.objects.get_mut(&oid) {
            object.prototyped = command.prototyped;
        }
        self.state.stack.push(StackItem {
            mana_colors_spent_to_cast: payment.mana_colors_spent,
            id: oid,
            controller: player,
            card_id: card_id.clone(),
            prototyped: command.prototyped,
            targets: stack_targets,
            ability_text: None,
            source_permanent_id: None,
            source_owner: None,
            source_zone_change: 0,
            trigger_use_key: None,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            activated_ability_occurrence: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: prepare_source.is_some(),
            chosen_x,
            face_index,
            chosen_modes,
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: None,
            cast_cost_receipts,
            payment_result,
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: payment.blight_receipts.clone(),
            // A spell's effects always act on its controller.
            trigger_context: TriggerContext::default(),
            cast_method,
            returned_attacker_assignment,
        });
        if let Some(source) = &prepare_source {
            if self
                .state
                .zone_change_generation
                .get(&source.object_id)
                .copied()
                .unwrap_or(0)
                == source.zone_change_generation
            {
                super::preparation::unprepare_permanent(&mut self.state, source.object_id);
            }
            if let Some(copy) = self.state.objects.get_mut(&oid) {
                copy.owner = player;
                copy.controller = player;
                copy.base_controller = player;
            }
        }

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
        let mut stack_annotation = match (needs_face_annotation, has_x) {
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
            batch
                .events
                .push(ev_log(format!("P{player} pays {life_paid} life.")));
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
                card_display_name: self
                    .registry
                    .get(&cast_card_id)
                    .and_then(|definition| definition.face_display_name(face_index))
                    .unwrap_or(&face_name)
                    .to_string(),
                is_prepare_spell: prepare_source.is_some(),
                is_copy: prepare_source.is_some(),
                is_triggered: false,
                copy_source_object_id: 0,
                chosen_mode_indices,
                chosen_mode_labels,
                chosen_cast_cost_labels,
                source_token_identity: None,
                primary_presentation: stack_presentation.primary,
                chosen_mode_presentations: stack_presentation.chosen_modes,
                chosen_cast_cost_presentations: stack_presentation.chosen_cast_costs,
                prototyped: command.prototyped,
                ability_controller_player_id: None,
                ability_source: None,
            })),
        });
        let fact = self.record_spell_cast(player, oid, origin, mana_value, mana_spent);
        target_triggers
            .extend(self.collect_mana_spending_triggers(&fact, &payment.restricted_mana_spent));
        self.record_committed_events(&crime_events);
        target_triggers.extend(self.collect_event_triggers(&crime_events));
        target_triggers.extend(
            self.collect_committed_cost_triggers(payment.trigger_events, payment.sacrificed),
        );
        target_triggers.extend(payment.expend_triggers);
        self.snapshot_completed_cast(oid);
        target_triggers.extend(self.collect_spell_keyword_triggers(&fact));
        target_triggers.extend(self.collect_delayed_copy_triggers(&fact));
        target_triggers.extend(self.collect_event_triggers(&[GameEvent::SpellCast { fact }]));
        // Both kinds of triggers are waiting when the cast completes, so they form one CR 603.3b
        // ordering group rather than forcing an artificial target-trigger/cast-trigger order.
        self.stage_triggers(target_triggers);
        batch.events.push(ev_priority_changed(self));
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    pub(super) fn active_mana_options(
        &self,
        permanent_id: ObjectId,
        ability: &tricerules_card_model::ActivatedAbilityDef,
    ) -> Option<Vec<tricerules_card_model::ManaAmount>> {
        if ability.opponent_land_mana_output() {
            return Some(self.opponent_land_mana_options(permanent_id, ability));
        }
        self.active_mana_options_for_fixed_output(permanent_id, ability)
    }

    /// Evaluate one land's current set of producible colors without checking activation costs or
    /// legality. Fixed and conditional mana abilities seed the set; opponent-land abilities then
    /// propagate those colors until no land gains another. A least fixed point keeps Orchard-only
    /// cycles empty.
    fn opponent_land_mana_options(
        &self,
        source_id: ObjectId,
        ability: &tricerules_card_model::ActivatedAbilityDef,
    ) -> Vec<tricerules_card_model::ManaAmount> {
        let mut lands = self
            .state
            .objects
            .values()
            .filter(|object| object.zone == Zone::Battlefield)
            .filter_map(|object| {
                let characteristics = self.characteristics(object.id)?;
                characteristics
                    .has_type("Land")
                    .then_some((object.id, characteristics.controller))
            })
            .collect::<Vec<_>>();
        lands.sort_by_key(|(object_id, _)| *object_id);

        let mut colors_by_land = lands
            .iter()
            .map(|(object_id, _)| (*object_id, 0u8))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut dynamic_lands = Vec::new();

        for (land_id, land_controller) in &lands {
            for effective in self.effective_activated_abilities(*land_id) {
                let ability = &effective.definition;
                if ability.opponent_land_mana_output() {
                    dynamic_lands.push((*land_id, *land_controller));
                } else if let Some(options) =
                    self.active_mana_options_for_fixed_output(*land_id, ability)
                {
                    for option in options {
                        let colors = colors_by_land.entry(*land_id).or_default();
                        *colors |= mana_color_mask(option);
                    }
                }
            }
        }
        dynamic_lands.sort_by_key(|(land_id, _)| *land_id);
        dynamic_lands.dedup_by_key(|(land_id, _)| *land_id);

        loop {
            let mut changed = false;
            for (land_id, land_controller) in &dynamic_lands {
                let mut opponent_colors = 0u8;
                for (other_land_id, other_controller) in &lands {
                    if self
                        .state
                        .are_opponents(*land_controller, *other_controller)
                    {
                        opponent_colors |= colors_by_land[other_land_id];
                    }
                }
                let colors = colors_by_land
                    .get_mut(land_id)
                    .expect("every dynamic mana land has a color entry");
                let next = *colors | opponent_colors;
                if next != *colors {
                    *colors = next;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        let Some(source_controller) = self
            .characteristics(source_id)
            .map(|characteristics| characteristics.controller)
        else {
            return vec![tricerules_card_model::ManaAmount::default()];
        };
        let mut available_colors = 0u8;
        for (land_id, land_controller) in &lands {
            if self
                .state
                .are_opponents(source_controller, *land_controller)
            {
                available_colors |= colors_by_land[land_id];
            }
        }

        let multiplier = self.tapped_permanent_mana_multiplier(source_id, ability);
        let options = (0..5)
            .filter(|index| available_colors & (1 << index) != 0)
            .map(|index| {
                multiply_mana_amount(mana_amount_for_color(color_for_index(index), 1), multiplier)
            })
            .collect::<Vec<_>>();
        if options.is_empty() {
            // CR 605.2: this remains an activatable mana ability that taps its source and adds 0.
            vec![tricerules_card_model::ManaAmount::default()]
        } else {
            options
        }
    }

    fn active_mana_options_for_fixed_output(
        &self,
        permanent_id: ObjectId,
        ability: &tricerules_card_model::ActivatedAbilityDef,
    ) -> Option<Vec<tricerules_card_model::ManaAmount>> {
        let default_options = ability.mana_options()?;
        let controller = self
            .characteristics(permanent_id)
            .map(|characteristics| characteristics.controller)?;
        let options = if ability.commander_color_identity_mana_output().is_some() {
            let identity = &self
                .state
                .players
                .iter()
                .find(|player| player.id == controller)?
                .color_identity;
            if identity.is_empty() {
                // CR 605.2 keeps this a mana ability even when the current game state supplies no
                // legal color; activating it still taps the source and adds 0.
                vec![tricerules_card_model::ManaAmount::default()]
            } else {
                identity
                    .iter()
                    .copied()
                    .map(|color| mana_amount_for_color(color, 1))
                    .collect()
            }
        } else {
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
                            stack_item: None,
                            previous_effect_result: None,
                        },
                    )
                })
                .map(|conditional| conditional.options.as_slice())
                .unwrap_or(default_options.as_slice())
                .to_vec()
        };
        let source_counter_multiplier = ability.mana_source_counter().map_or(1, |counter| {
            self.state
                .objects
                .get(&permanent_id)
                .filter(|object| object.zone == Zone::Battlefield)
                .map(|object| object.counter_count(counter))
                .unwrap_or(0)
        });
        let multiplier = source_counter_multiplier
            .saturating_mul(self.tapped_permanent_mana_multiplier(permanent_id, ability));
        Some(
            options
                .into_iter()
                .map(|amount| multiply_mana_amount(amount, multiplier))
                .collect(),
        )
    }

    /// CR 106.12 / 614.1: replacement effects for tapping a permanent for mana apply only when
    /// the activated mana ability has a tap symbol in its activation cost. Counter-paid storage
    /// abilities without a tap cost are deliberately excluded.
    fn tapped_permanent_mana_multiplier(
        &self,
        permanent_id: ObjectId,
        ability: &tricerules_card_model::ActivatedAbilityDef,
    ) -> u32 {
        if !ability
            .costs
            .iter()
            .any(|cost| matches!(cost, AbilityCost::Tap))
        {
            return 1;
        }
        let Some(controller) = self
            .state
            .objects
            .get(&permanent_id)
            .filter(|source| source.zone == Zone::Battlefield)
            .map(|source| source.controller)
        else {
            return 1;
        };

        let mut sources: Vec<_> = self
            .state
            .objects
            .values()
            .filter(|object| object.zone == Zone::Battlefield && object.controller == controller)
            .map(|object| object.id)
            .collect();
        sources.sort_unstable();
        sources
            .into_iter()
            .flat_map(|source| self.active_static_ability_definitions(source))
            .filter_map(|static_ability| {
                match static_ability {
                tricerules_card_model::primitives::StaticAbilityDef::MultiplyManaFromTappedPermanents {
                    multiplier,
                } => Some(multiplier),
                _ => None,
            }
            })
            .fold(1_u32, u32::saturating_mul)
    }

    /// Validate the bounded storage-land choice identically for payment previews and committed
    /// activations. The maximum is derived from the live source and its color options are
    /// card-authored, so request fields cannot supply either value.
    pub(in crate::engine) fn validate_activation_mana_choice(
        &self,
        source: ObjectId,
        source_zone: AbilitySourceZone,
        ability: &ActivatedAbilityDef,
        mana_option_index: u32,
        x_value: u32,
        first_color_count: u32,
    ) -> Result<(), EngineError> {
        let has_loyalty_x_cost = ability.costs.contains(&AbilityCost::LoyaltyX);
        let has_mana_x_cost = ability.costs.iter().any(|cost| match cost {
            AbilityCost::Mana(mana) | AbilityCost::Waterbend(mana) => mana.has_x(),
            _ => false,
        });
        let has_storage_x_cost = ability
            .costs
            .contains(&AbilityCost::RemoveXStorageCountersFromSource);
        if has_loyalty_x_cost {
            if source_zone != AbilitySourceZone::Battlefield
                || has_mana_x_cost
                || has_storage_x_cost
                || ability.storage_counter_split_mana().is_some()
            {
                return Err(EngineError::Illegal(
                    "LoyaltyX cannot share the activated-ability X choice with another cost",
                ));
            }
            let actor = self
                .controller_of(source)
                .ok_or(EngineError::Illegal("LoyaltyX source is unavailable"))?;
            let range = self
                .loyalty_x_range(actor, source)
                .ok_or(EngineError::Illegal("LoyaltyX has no legal range"))?;
            if range.delta_for(x_value).is_none() {
                return Err(EngineError::Illegal("loyalty X is outside the legal range"));
            }
        }

        if ability.storage_counter_split_mana().is_some() {
            let source_object = self
                .state
                .objects
                .get(&source)
                .filter(|object| object.zone == Zone::Battlefield)
                .ok_or(EngineError::Illegal("storage mana source missing"))?;
            if source_zone != AbilitySourceZone::Battlefield
                || mana_option_index != 0
                || x_value
                    > source_object.counter_count(tricerules_card_model::CounterKind::Storage)
                || first_color_count > x_value
            {
                return Err(EngineError::Illegal(
                    "invalid storage mana X or color split",
                ));
            }
            return Ok(());
        }

        if first_color_count != 0
            || (x_value != 0
                && !has_loyalty_x_cost
                && !ability
                    .costs
                    .iter()
                    .any(|cost| matches!(cost, AbilityCost::Mana(mana) if mana.has_x())))
        {
            return Err(EngineError::Illegal("unexpected mana X or color split"));
        }
        Ok(())
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

    /// Validate and capture one modal activation declaration without mutating engine state.
    pub(in crate::engine) fn validate_and_capture_modal_activation(
        &self,
        player: PlayerId,
        target_source: TargetSourceIdentity,
        modal: &ModalDef,
        command: &rv1::ActivateAbility,
    ) -> Result<(Vec<rv1::TargetRef>, Vec<ChosenMode>), EngineError> {
        if !command.targets.is_empty() {
            return Err(EngineError::Illegal(
                "modal activated abilities use selected_modes targets",
            ));
        }
        let mut seen = HashSet::new();
        for selection in &command.selected_modes {
            if !seen.insert(selection.mode_index) {
                return Err(EngineError::Illegal("a mode may be selected only once"));
            }
        }
        if command.selected_modes.len() < modal.min_modes as usize
            || command.selected_modes.len() > modal.max_modes as usize
        {
            return Err(EngineError::Illegal("illegal number of selected modes"));
        }
        let mut ordered: Vec<_> = command.selected_modes.iter().collect();
        ordered.sort_by_key(|selection| selection.mode_index);
        let mut public_targets = Vec::new();
        let mut captured_modes = Vec::with_capacity(ordered.len());
        for selection in ordered {
            let mode = modal
                .modes
                .get(selection.mode_index as usize)
                .ok_or(EngineError::Illegal("bad activated ability mode index"))?;
            validate_modal_permanent_target_generations(self, &selection.targets)?;
            validate_ability_targets_with_context_and_program(
                self,
                player,
                target_source,
                &selection.targets,
                TargetResolutionContext {
                    effects: &mode.effects,
                    targeting: mode.targeting.as_ref(),
                    trigger_context: TriggerContext::default(),
                    program: None,
                },
            )?;
            validate_x_target_mana_value(
                self,
                &mode.effects,
                mode.targeting.as_ref(),
                &selection.targets,
                command.x_value,
                None,
            )?;
            public_targets.extend(selection.targets.iter().cloned());
            captured_modes.push(ChosenMode {
                mode_id: mode.mode_id.clone(),
                targets: selection
                    .targets
                    .iter()
                    .map(|target| capture_stack_target(self, target))
                    .collect(),
            });
        }
        Ok((public_targets, captured_modes))
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
        let x_value = command.x_value;
        let mana_split_first_color_count = command.mana_split_first_color_count;
        let selected_modes = command.selected_modes.as_slice();
        let resolving_mana_payment = self.resolving_mana_payment_for(player);
        let attack_mana_payment = self
            .state
            .pending_attack_declaration
            .as_ref()
            .is_some_and(|pending| pending.attacking_player_id == player);
        if self.state.priority_player_id() != player
            && !resolving_mana_payment
            && !attack_mana_payment
        {
            return Err(EngineError::Illegal("not your priority"));
        }
        if self.state.turn_step == TurnStep::Cleanup
            && !self.state.cleanup_priority_active
            && !resolving_mana_payment
            && !attack_mana_payment
        {
            return Err(EngineError::Illegal("no abilities during cleanup"));
        }
        if priority_locked_for_combat_declaration(&self.state) && !attack_mana_payment {
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

        let (card_id, face_up_index, effective) = match source_zone {
            AbilitySourceZone::Battlefield => {
                let (card_id, face_index) = self
                    .effective_card_identity(permanent_id)
                    .map(|(card_id, face_index)| (card_id.to_string(), face_index))
                    .ok_or(EngineError::Illegal("bad face index on permanent"))?;
                let effective = self
                    .effective_activated_abilities(permanent_id)
                    .into_iter()
                    .find(|effective| effective.slot == command.ability_index)
                    .ok_or(EngineError::Illegal("no such activated ability"))?;
                (card_id, face_index, effective)
            }
            AbilitySourceZone::Hand | AbilitySourceZone::Graveyard => {
                let card_id = object.card_id.clone();
                let (ability, face_index) = self
                    .authored_zone_activated_abilities(permanent_id, source_zone)
                    .into_iter()
                    .find(|(index, _, _)| *index == ability_index)
                    .map(|(_, ability, face_index)| (ability, face_index))
                    .ok_or(EngineError::Illegal("no such activated ability"))?;
                let definition = self.ability_definition(
                    permanent_id,
                    face_index,
                    vec![ability.ability_id.clone()],
                );
                (
                    card_id,
                    face_index,
                    EffectiveActivatedAbility::authored(command.ability_index, ability, definition),
                )
            }
        };
        let ability = effective.definition.clone();
        if ability.modal.is_none() && !selected_modes.is_empty() {
            return Err(EngineError::Illegal(
                "selected_modes given for a nonmodal activated ability",
            ));
        }
        if source_zone == AbilitySourceZone::Hand
            && ability.costs.contains(&AbilityCost::ReturnTappedCreature)
        {
            return Err(EngineError::Illegal(
                "hand ability with a tapped-creature return cost requires staged activation",
            ));
        }
        if ability.requires_opponent_target_choice() {
            return Err(EngineError::Illegal(
                "opponent target choices require staged activation",
            ));
        }
        self.validate_activation_mana_choice(
            permanent_id,
            source_zone,
            &ability,
            mana_option_index,
            x_value,
            mana_split_first_color_count,
        )?;
        if source_zone == AbilitySourceZone::Battlefield
            && self.activated_abilities_prohibited(permanent_id)
        {
            return Err(EngineError::Illegal(
                "activated abilities of this permanent can't be activated",
            ));
        }
        let casting_mana_payment = self
            .state
            .pending_spell_cast
            .as_ref()
            .is_some_and(|pending| pending.caster == player)
            || attack_mana_payment
            || self.paying_ability_activation(player);
        if resolving_mana_payment || casting_mana_payment {
            if !ability.is_mana_ability() {
                return Err(EngineError::Illegal(
                    "only mana abilities may be activated during payment",
                ));
            }
        } else if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        if source_zone != AbilitySourceZone::Battlefield && ability.is_mana_ability() {
            return Err(EngineError::Illegal(
                "nonbattlefield mana abilities are not supported",
            ));
        }

        // CR 602.5d / 702.6a: explicit activation instructions and equip both use the shared
        // sorcery-speed window.
        if ability.requires_sorcery_speed()
            && !super::priority::sorcery_speed_available(&self.state, player)
            && !self.planeswalker_loyalty_instant_timing_allowed(permanent_id, player, &ability)
        {
            return Err(EngineError::Illegal("ability only at sorcery speed"));
        }
        if !self.activation_conditions_hold(permanent_id, &ability) {
            return Err(EngineError::Illegal("activation condition not met"));
        }
        if !self.activation_limit_allows(permanent_id, ability_index, &ability) {
            return Err(EngineError::Illegal("activation limit reached"));
        }

        if ability.is_mana_ability() {
            let mut batch = self.resolve_mana_ability(
                player,
                idx,
                permanent_id,
                &card_id,
                &effective,
                mana_option_index,
                targets,
                flex_payments,
                cost_selections,
                restricted_mana,
                x_value,
                mana_split_first_color_count,
                command.payment.as_ref(),
            )?;
            if resolving_mana_payment {
                if let Some(event) = self.resolution_payment_choice_event() {
                    batch.events.push(event);
                }
            }
            return Ok(batch);
        }

        // CR 602.2b / 700.2: modal activated abilities choose a mode as part of activation.
        // Resolve the positional wire coordinate to its stable authored identity before costs,
        // and keep each mode's targets separate from the other mode bodies.
        let target_source =
            TargetSourceIdentity::captured_for_ability(permanent_id, source_zone_change, &ability);
        let program = self.program_for_activation_occurrence(&effective.occurrence);
        let (activation_targets, chosen_modes) = if let Some(modal) = ability.modal.as_ref() {
            self.validate_and_capture_modal_activation(player, target_source, modal, command)?
        } else {
            if !selected_modes.is_empty() {
                return Err(EngineError::Illegal(
                    "selected_modes given for a nonmodal activated ability",
                ));
            }
            // CR 608.2: an ability's effect list validates exactly like a spell's — same shape,
            // same multi-target handling — so it goes through the list-level entry point.
            validate_ability_targets_with_context_and_program(
                self,
                player,
                target_source,
                targets,
                TargetResolutionContext {
                    effects: &ability.effect,
                    targeting: ability.targeting.as_ref(),
                    trigger_context: TriggerContext::default(),
                    program,
                },
            )?;
            validate_x_target_mana_value(
                self,
                &ability.effect,
                ability.targeting.as_ref(),
                targets,
                x_value,
                program,
            )?;
            (targets.to_vec(), Vec::new())
        };

        let announcement = self.capture_ability_announcement(
            player,
            permanent_id,
            &effective,
            &activation_targets,
            x_value,
            (card_id.clone(), face_up_index),
        )?;
        let targeting_cost = self.targeting_cost_increase(
            player,
            TargetingCostAction::ActivatedAbilities,
            &activation_targets,
        );
        let mana_reduction = self.activated_mana_reduction(player, permanent_id, &ability)?;
        let prepared_payment = self.prepare_ability_costs(
            player,
            idx,
            permanent_id,
            &ability.costs,
            flex_payments,
            cost_selections,
            restricted_mana,
            x_value,
            targeting_cost,
            mana_reduction,
        )?;
        let cost_plan = self.finish_ability_payment(
            player,
            permanent_id,
            prepared_payment,
            command.payment.as_ref(),
        )?;
        let mut announcement = announcement;
        announcement.chosen_modes = chosen_modes;
        if !announcement.chosen_modes.is_empty() {
            announcement.stack_targets.clear();
        }
        let replay = crate::state::ActivationCostReplay {
            actor: player,
            source_object_id: permanent_id,
            source_zone_change_generation: source_zone_change,
            command_kind: crate::state::ActivationCostCommandKind::Direct,
            command_bytes: command.encode_to_vec(),
            transaction_id: None,
            expected_revision: None,
            counter_debits: Vec::new(),
        };
        self.finish_ability_cost_transaction(cost_plan, announcement, replay)
    }

    pub(super) fn finish_ability_cost_transaction(
        &mut self,
        plan: super::payment::transaction::CostTransactionPlan,
        announcement: AnnouncedAbilityActivation,
        mut replay: crate::state::ActivationCostReplay,
    ) -> Result<RuledEventBatch, EngineError> {
        let plan = self.prepare_cost_transaction_commit(plan)?;
        let counter_costs = plan.counter_placement_costs();
        if counter_costs.is_empty() {
            let payment = self.commit_cost_transaction(plan)?;
            self.complete_activation_cost_state(&replay);
            return Ok(self.finish_announced_ability(announcement, payment));
        }

        replay.counter_debits = counter_costs
            .iter()
            .map(|debit| crate::state::ActivationCostCounterDebit {
                debit_index: debit.debit_index,
                object_id: debit.object_id,
                zone_change_generation: debit.generation,
                counter_kind: debit.kind,
                requested_count: debit.count,
            })
            .collect();
        if replay.actor != plan.player()
            || counter_costs.iter().any(|debit| {
                debit.kind == tricerules_card_model::CounterKind::Loyalty
                    && (debit.object_id != replay.source_object_id
                        || debit.generation != replay.source_zone_change_generation)
            })
        {
            return Err(EngineError::Illegal(
                "counter cost replay does not match its activation",
            ));
        }
        let mut placements = Vec::with_capacity(counter_costs.len());
        for debit in &counter_costs {
            let mut placement = self
                .proposed_object_counter_placement(
                    replay.actor,
                    debit.object_id,
                    [(debit.kind, debit.count)],
                    super::continuous::CounterPlacementOrigin::Cost,
                )
                .ok_or(EngineError::Illegal(
                    "positive loyalty cost recipient became stale",
                ))?;
            placement.cost_debit_index = Some(debit.debit_index);
            placements.push(placement);
        }
        let (batch, candidates) = self.prepare_counter_placement_batch(placements)?;
        if let Some(candidates) = candidates {
            let completion = crate::state::CounterPlacementCompletion::ActivationCost {
                replay: replay.clone(),
            };
            self.pending_counter_cost_internal = Some(activation::PendingCounterCostInternal {
                replay,
                plan,
                announcement,
            });
            let mut events = Vec::new();
            self.park_counter_placement_choice(batch, candidates, None, completion, &mut events);
            return Ok(finish_with_events(self, events));
        }

        let payment = self.commit_cost_transaction_with_counter_placements(plan, batch.events)?;
        self.complete_activation_cost_state(&replay);
        Ok(self.finish_announced_ability(announcement, payment))
    }

    pub(super) fn complete_activation_cost_state(
        &mut self,
        replay: &crate::state::ActivationCostReplay,
    ) {
        if replay.command_kind == crate::state::ActivationCostCommandKind::Staged {
            self.state.pending_ability_activation = None;
            self.pending_ability_activation_internal = None;
        }
        self.pending_counter_cost_internal = None;
    }

    pub(super) fn finish_activation_cost_replacements(
        &mut self,
        replay: &crate::state::ActivationCostReplay,
        placements: Vec<super::replacement::ProposedCounterPlacement>,
    ) -> Result<RuledEventBatch, EngineError> {
        let pending = self
            .pending_counter_cost_internal
            .as_ref()
            .filter(|pending| pending.replay == *replay)
            .cloned()
            .ok_or(EngineError::Illegal(
                "activation counter-cost snapshot is stale",
            ))?;
        if replay.actor != pending.plan.player()
            || pending.announcement.source_identity()
                != (
                    replay.actor,
                    replay.source_object_id,
                    replay.source_zone_change_generation,
                )
            || self
                .state
                .zone_change_generation
                .get(&replay.source_object_id)
                .copied()
                .unwrap_or(0)
                != replay.source_zone_change_generation
        {
            return Err(EngineError::Illegal(
                "activation source changed during counter-cost replacement",
            ));
        }
        match replay.command_kind {
            crate::state::ActivationCostCommandKind::Direct => {
                let command = rv1::ActivateAbility::decode(replay.command_bytes.as_slice())
                    .map_err(|_| EngineError::Illegal("accepted activation command is invalid"))?;
                if command.source_object_id != replay.source_object_id
                    || command.expected_zone_change_generation
                        != replay.source_zone_change_generation
                    || self.state.pending_ability_activation.is_some()
                {
                    return Err(EngineError::Illegal(
                        "direct activation identity changed during counter-cost replacement",
                    ));
                }
            }
            crate::state::ActivationCostCommandKind::Staged => {
                let command = rv1::CommitAbilityActivation::decode(replay.command_bytes.as_slice())
                    .map_err(|_| EngineError::Illegal("accepted activation payment is invalid"))?;
                let state_matches =
                    self.state
                        .pending_ability_activation
                        .as_ref()
                        .is_some_and(|pending| {
                            pending.actor_player_id == replay.actor
                                && pending.source_object_id == replay.source_object_id
                                && pending.source_zone_change_generation
                                    == replay.source_zone_change_generation
                                && Some(pending.transaction_id) == replay.transaction_id
                                && Some(pending.revision) == replay.expected_revision
                        });
                if !state_matches
                    || Some(command.transaction_id) != replay.transaction_id
                    || Some(command.expected_revision) != replay.expected_revision
                {
                    return Err(EngineError::Illegal(
                        "staged activation transaction changed during counter-cost replacement",
                    ));
                }
            }
        }
        let (plan, permission) =
            self.prepare_cost_transaction_commit_with_permission(pending.plan.clone())?;
        let signatures = plan
            .counter_placement_costs()
            .iter()
            .map(|debit| crate::state::ActivationCostCounterDebit {
                debit_index: debit.debit_index,
                object_id: debit.object_id,
                zone_change_generation: debit.generation,
                counter_kind: debit.kind,
                requested_count: debit.count,
            })
            .collect::<Vec<_>>();
        if signatures != replay.counter_debits {
            return Err(EngineError::Illegal(
                "counter cost debits changed during replacement order",
            ));
        }
        if permission == super::payment::transaction::CostPermissionStatus::MissingAnyColor {
            // The answer itself was valid, but its exact retained debit lost the live permission.
            // Complete the cancellation as a logged action only after all other checks succeed.
            self.validate_resolved_cost_counter_placements(&plan, &placements)?;
            self.complete_activation_cost_state(replay);
            return Ok(finish_with_events(self, vec![ev_log(format!(
                "P{}'s activation was cancelled because the selected mana can no longer pay its cost.",
                replay.actor,
            ))]));
        }
        let payment = self.commit_cost_transaction_with_counter_placements(plan, placements)?;
        self.complete_activation_cost_state(replay);
        Ok(self.finish_announced_ability(pending.announcement, payment))
    }

    pub(super) fn capture_ability_announcement(
        &self,
        player: PlayerId,
        permanent_id: ObjectId,
        effective: &EffectiveActivatedAbility,
        targets: &[rv1::TargetRef],
        x_value: u32,
        source_identity: (String, usize),
    ) -> Result<AnnouncedAbilityActivation, EngineError> {
        let object = self
            .state
            .objects
            .get(&permanent_id)
            .ok_or(EngineError::Illegal("activation source missing"))?;
        let concealed = object.zone == Zone::Battlefield && object.face_down;
        let (card_id, face_up_index) = source_identity;
        let source_token_identity =
            if !concealed && object.zone == Zone::Battlefield && object.is_token() {
                self.copiable_values_for(permanent_id)
                    .map(|values| super::resolution::token_identity(&values))
            } else {
                None
            };
        let card_name = if concealed {
            "Face-down permanent".to_string()
        } else {
            source_token_identity
                .as_ref()
                .map(|identity| identity.name.clone())
                .filter(|name| !name.is_empty())
                .or_else(|| {
                    self.registry
                        .get(&card_id)
                        .map(|definition| definition.name.clone())
                })
                .unwrap_or_else(|| card_id.clone())
        };
        let ability = effective.definition.clone();
        let loyalty_authorization = if ability.is_loyalty_ability() {
            Some(
                self.loyalty_activation_authorization(permanent_id, player)
                    .ok_or(EngineError::Illegal("activation limit reached"))?,
            )
        } else {
            None
        };
        let ability_text = ability.fallback_text_with_path(&card_name, &effective.ability_path());
        let primary_presentation = (!concealed)
            .then_some(effective.presentation_definition.as_ref())
            .flatten()
            .map(|definition| {
                ability_presentation(
                    self.registry,
                    definition,
                    &ability.presentation,
                    ability_text.clone(),
                )
            });
        let mut snapshot = AnnouncedAbilityActivation {
            player,
            permanent_id,
            source_owner: object.owner,
            source_zone_change: self.payment_object_ref(permanent_id).zone_change_generation,
            source_linked_exile_occurrence: self.linked_exile_occurrence(permanent_id),
            source_face_change: self
                .state
                .face_change_generation
                .get(&permanent_id)
                .copied()
                .unwrap_or(0),
            ability_index: effective.slot as usize,
            card_id,
            face_up_index,
            ability_text,
            activated_ability_occurrence: effective.occurrence.clone(),
            source_token_identity,
            card_name,
            activation_reveal_id: None,
            primary_presentation,
            targets: Vec::new(),
            stack_targets: Vec::new(),
            chosen_modes: Vec::new(),
            reserved_virtual_id: self.state.next_object_id,
            crime_events: Vec::new(),
            target_triggers: Vec::new(),
            activation_uses: self.limited_activation_uses(
                permanent_id,
                &effective.occurrence,
                &ability,
            ),
            loyalty_authorization,
            planeswalker_loyalty_activation: ability.is_loyalty_ability()
                && object.zone == Zone::Battlefield
                && self
                    .characteristics(permanent_id)
                    .is_some_and(|c| c.has_type("Planeswalker")),
            ability,
            x_value,
        };
        self.append_ability_announcement_targets(&mut snapshot, targets, None);
        Ok(snapshot)
    }

    pub(super) fn append_ability_announcement_targets(
        &self,
        snapshot: &mut AnnouncedAbilityActivation,
        targets: &[rv1::TargetRef],
        required_controller: Option<PlayerId>,
    ) {
        if targets.is_empty() {
            return;
        }
        let captured: Vec<_> = targets
            .iter()
            .map(|target| {
                let mut receipt = capture_stack_target(self, target);
                receipt.required_controller = required_controller;
                receipt
            })
            .collect();
        snapshot
            .target_triggers
            .extend(self.collect_event_triggers(&[GameEvent::TargetsChosen {
                controller: snapshot.player,
                source: TargetingSourceKind::Ability,
                stack_object: StackObjectRef {
                    object_id: snapshot.reserved_virtual_id,
                    zone_change_generation: None,
                },
                targets: captured.clone(),
            }]));
        snapshot
            .crime_events
            .extend(self.crime_event(snapshot.player, targets));
        snapshot.targets.extend_from_slice(targets);
        snapshot.stack_targets.extend(captured);
    }

    /// Infallible publication after all guards and atomic cost commitment have succeeded.
    pub(super) fn finish_announced_ability(
        &mut self,
        announcement: AnnouncedAbilityActivation,
        payment: super::payment::transaction::CostPaymentReceipt,
    ) -> RuledEventBatch {
        let AnnouncedAbilityActivation {
            player,
            permanent_id,
            source_owner,
            source_zone_change,
            source_linked_exile_occurrence,
            source_face_change,
            ability_index,
            card_id,
            face_up_index,
            ability,
            activated_ability_occurrence,
            ability_text,
            source_token_identity,
            card_name,
            activation_reveal_id,
            primary_presentation,
            targets,
            stack_targets,
            chosen_modes,
            reserved_virtual_id,
            crime_events,
            mut target_triggers,
            activation_uses,
            loyalty_authorization,
            planeswalker_loyalty_activation,
            x_value,
        } = announcement;
        let ability_annotation = if ability.costs.contains(&AbilityCost::LoyaltyX) {
            format!("{ability_text} (X = {x_value})")
        } else {
            ability_text.clone()
        };
        let idx = self
            .state
            .player_idx(player)
            .expect("validated activation actor");
        let trefs: Vec<_> = targets.iter().map(|target| target.object_id).collect();
        let (chosen_mode_indices, chosen_mode_labels, chosen_mode_presentations) =
            activated_stack_mode_presentation(
                &ability,
                &ability_text,
                &chosen_modes,
                primary_presentation.as_ref(),
            )
            .expect("announced activated modes remain in their captured definition");
        self.state.undoable_mana_abilities.clear();
        self.record_limited_activations(activation_uses.clone());
        if let Some(LoyaltyActivationAuthorization::ChainVeil {
            permission_id,
            walker,
            generation,
        }) = loyalty_authorization
        {
            let permission = self
                .state
                .chain_veil_permissions
                .iter_mut()
                .find(|permission| permission.id == permission_id && permission.player == player)
                .expect("captured Chain Veil permission survives committed costs");
            debug_assert!(!permission.used_walkers.contains(&(walker, generation)));
            permission.used_walkers.push((walker, generation));
        }
        if planeswalker_loyalty_activation {
            self.state
                .turn_history
                .current
                .player_mut(player)
                .activated_planeswalker_loyalty = true;
        }

        // Allocate the ability's stack identity now that every cost has committed. Triggers
        // staged while paying consumed ids from the same stream, so the provisional reservation
        // may already have been taken; re-point pre-payment target watchers at the real identity.
        let virtual_id = self.state.next_object_id;
        self.state.next_object_id += 1;
        if virtual_id != reserved_virtual_id {
            for trigger in &mut target_triggers {
                if let Some(stack_object) = trigger.trigger_context.targeting_stack_object.as_mut()
                {
                    if stack_object.object_id == reserved_virtual_id {
                        stack_object.object_id = virtual_id;
                    }
                }
            }
        }

        self.state.stack_presentations.insert(
            virtual_id,
            StackPresentation {
                source_label: Some(card_name.clone()),
                activation_reveal_id,
                primary: primary_presentation.clone(),
                chosen_modes: chosen_mode_presentations.clone(),
                source_token_identity: source_token_identity.clone(),
                ..Default::default()
            },
        );
        self.state.stack.push(StackItem {
            mana_colors_spent_to_cast: Default::default(),
            id: virtual_id,
            controller: player,
            card_id: card_id.clone(),
            prototyped: false,
            targets: stack_targets,
            ability_text: Some(ability_annotation.clone()),
            source_permanent_id: Some(permanent_id),
            source_owner: Some(source_owner),
            source_zone_change,
            trigger_use_key: None,
            source_face_change,
            ability_index: Some(ability_index),
            activated_ability: Some(ability.clone()),
            activated_ability_occurrence: Some(activated_ability_occurrence),
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            chosen_x: x_value,
            face_index: face_up_index,
            chosen_modes,
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: None,
            cast_cost_receipts: vec![],
            payment_result: CardResultCohort {
                cards: payment
                    .paid_card_costs
                    .iter()
                    .map(|cost| cost.result().clone())
                    .collect(),
                program_discard_result: None,
            },
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: payment.blight_receipts.clone(),
            // An activated ability's effects act on the player who activated it.
            trigger_context: TriggerContext {
                linked_exile_occurrence: source_linked_exile_occurrence,
                ..TriggerContext::default()
            },
            cast_method: SpellCastMethod::Normal,
            returned_attacker_assignment: payment.returned_attacker_assignment,
        });
        self.state.passes_since_stack_change = 0;
        self.state.priority_idx = idx;

        let tgt_line = format_spell_targets_log(&self.state, self.registry, &trefs);
        let ability_log_suffix = if ability.costs.contains(&AbilityCost::LoyaltyX) {
            format!("{tgt_line} (X = {x_value})")
        } else {
            tgt_line
        };
        let paid_costs_line = format_paid_card_costs_log(&payment.paid_card_costs);
        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log_ability(
            format!("P{player} activates {card_name}{paid_costs_line}: "),
            &ability_text,
            primary_presentation.clone(),
            ability_log_suffix,
        ));
        for replacement_log in &payment.counter_replacement_logs {
            batch.events.push(ev_log(replacement_log.clone()));
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
                "P{player} pays {} life.",
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
                targets,
                ability_annotation,
                card_id: String::new(),
                card_display_name: String::new(),
                is_prepare_spell: false,
                is_copy: false,
                is_triggered: false,
                copy_source_object_id: 0,
                chosen_mode_indices,
                chosen_mode_labels,
                chosen_cast_cost_labels: vec![],
                source_token_identity,
                primary_presentation,
                chosen_mode_presentations,
                chosen_cast_cost_presentations: vec![],
                prototyped: false,
                ability_controller_player_id: Some(player),
                ability_source: self.public_stack_ability_source(permanent_id, source_zone_change),
            })),
        });
        // CR 603.3b: a trigger that fired while the cost was being paid goes on the stack *above*
        // the ability it paid for, so its events must follow that ability's StackPushed. Emitting
        // the trigger prompt first also made the client discard it, because an activated ability
        // reaching the stack is its signal that a pending trigger target was just answered.
        self.record_committed_events(&crime_events);
        target_triggers.extend(self.collect_event_triggers(&crime_events));
        target_triggers.extend(
            self.collect_committed_cost_triggers(payment.trigger_events, payment.sacrificed),
        );
        target_triggers.extend(self.collect_event_triggers(&[GameEvent::AbilityActivated {
            controller: player,
            stack_object: StackObjectRef {
                object_id: virtual_id,
                zone_change_generation: None,
            },
        }]));
        self.stage_triggers(target_triggers);
        batch.events.push(ev_priority_changed(self));
        fill_legal(&mut batch, self);
        batch
    }

    /// Teferi's emblem gives its controller a broad loyalty permission, while Teferi, Master of
    /// Time's static definition gives the permission only to its own active loyalty abilities.
    /// Explicit SorcerySpeed and Equip instructions retain their own timing restriction.
    fn planeswalker_loyalty_instant_timing_allowed(
        &self,
        permanent_id: ObjectId,
        actor: PlayerId,
        ability: &tricerules_card_model::ActivatedAbilityDef,
    ) -> bool {
        if !ability.is_loyalty_ability()
            || ability.timing == ActivationTiming::SorcerySpeed
            || ability.is_equip()
            || !super::priority::instant_timing_step_allowed(&self.state)
        {
            return false;
        }
        if !self
            .state
            .objects
            .get(&permanent_id)
            .is_some_and(|object| object.zone == Zone::Battlefield)
        {
            return false;
        }
        let Some(characteristics) = self.characteristics(permanent_id) else {
            return false;
        };
        if characteristics.controller != actor {
            return false;
        }
        let emblem_permission = characteristics.has_type("Planeswalker")
            && self.state.static_emblems.iter().any(|emblem| {
                emblem.controller == actor
                    && emblem
                        .effects
                        .contains(&StaticEmblemEffect::PlaneswalkerLoyaltyInstantTiming)
            });
        let source_permission = self
            .active_static_ability_definitions(permanent_id)
            .iter()
            .any(|definition| {
                matches!(
                    definition,
                    StaticAbilityDef::SourceLoyaltyAbilitiesAtInstantTiming
                )
            });
        emblem_permission || source_permission
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
        ability: &tricerules_card_model::ActivatedAbilityDef,
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
        if ability.source_zone == AbilitySourceZone::Battlefield
            && self.activated_abilities_prohibited(permanent_id)
        {
            return false;
        }
        let activating_player = if ability.source_zone == AbilitySourceZone::Battlefield {
            object.controller
        } else {
            object.owner
        };
        let timing_actor = if ability.source_zone == AbilitySourceZone::Battlefield {
            self.controller_of(permanent_id)
                .unwrap_or(activating_player)
        } else {
            activating_player
        };
        if ability.requires_sorcery_speed()
            && !super::priority::sorcery_speed_available(&self.state, activating_player)
            && !self.planeswalker_loyalty_instant_timing_allowed(
                permanent_id,
                timing_actor,
                ability,
            )
        {
            return false;
        }
        if !self.activation_conditions_hold(permanent_id, ability) {
            return false;
        }
        if !self.counter_costs_payable(permanent_id, &ability.costs) {
            return false;
        }
        if ability
            .costs
            .iter()
            .any(|cost| matches!(cost, AbilityCost::ReturnUnblockedAttacker))
            && self
                .ninjutsu_return_candidates(activating_player)
                .is_empty()
        {
            return false;
        }
        if ability.costs.contains(&AbilityCost::ReturnTappedCreature)
            && self
                .tapped_creature_return_candidates(activating_player)
                .is_empty()
            && !self.mana_ability_can_tap_creature_for_return(activating_player)
        {
            return false;
        }
        let life_cost = self.activated_life_cost(activating_player, &ability.costs);
        let available_life = self
            .state
            .player_idx(activating_player)
            .map(|index| self.state.players[index].life.max(0) as u32)
            .unwrap_or(0);
        if life_cost.is_none_or(|cost| cost > available_life) {
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
        if let Some(printed) = ability.costs.iter().find_map(|cost| match cost {
            AbilityCost::Loyalty(delta) => Some(*delta),
            _ => None,
        }) {
            let actor = self
                .controller_of(permanent_id)
                .unwrap_or(activating_player);
            let Some(delta) = self.effective_loyalty_cost(actor, permanent_id, printed) else {
                return false;
            };
            if (delta > 0 && !self.can_receive_counters(permanent_id))
                || (delta < 0 && object.counter_count(CounterKind::Loyalty) < delta.unsigned_abs())
            {
                return false;
            }
        }
        true
    }

    pub(super) fn tapped_creature_return_candidates(&self, player: PlayerId) -> Vec<ObjectId> {
        let Some(player_idx) = self.state.player_idx(player) else {
            return Vec::new();
        };
        self.state.players[player_idx]
            .battlefield
            .iter()
            .copied()
            .filter(|oid| {
                self.state.objects.get(oid).is_some_and(|object| {
                    object.zone == Zone::Battlefield && object.controller == player && object.tapped
                }) && self
                    .characteristics(*oid)
                    .is_some_and(|characteristics| characteristics.is_creature())
            })
            .collect()
    }

    fn mana_ability_can_tap_creature_for_return(&self, player: PlayerId) -> bool {
        let Some(player_idx) = self.state.player_idx(player) else {
            return false;
        };
        let battlefield = self.state.players[player_idx].battlefield.clone();
        let untapped_creatures: Vec<_> = battlefield
            .iter()
            .copied()
            .filter(|oid| {
                self.state.objects.get(oid).is_some_and(|object| {
                    object.zone == Zone::Battlefield
                        && object.controller == player
                        && !object.tapped
                }) && self
                    .characteristics(*oid)
                    .is_some_and(|characteristics| characteristics.is_creature())
            })
            .collect();

        battlefield.into_iter().any(|source_id| {
            let Some(source) = self.state.objects.get(&source_id) else {
                return false;
            };
            if source.zone != Zone::Battlefield || source.controller != player {
                return false;
            }
            self.effective_activated_abilities(source_id)
                .into_iter()
                .any(|effective| {
                    let ability = &effective.definition;
                    if !ability.is_mana_ability()
                        || ability.source_zone != AbilitySourceZone::Battlefield
                        || ability.costs.contains(&AbilityCost::ReturnTappedCreature)
                    {
                        return false;
                    }

                    // Reuse the engine's complete announced-cost candidate and payability
                    // calculation, including conditions, tap readiness, activation limits,
                    // and all selected-object costs.
                    let costs = super::legal_actions::legal_ability_cost_choices(
                        self,
                        player,
                        source_id,
                        effective.slot as usize,
                        ability,
                    );
                    if !costs.non_mana_costs_payable {
                        return false;
                    }

                    // A creature's own {T} mana ability remains the direct path. Other mana
                    // abilities may tap the creature through a TapPermanents cost, as with
                    // Springleaf Drum.
                    if ability.costs.contains(&AbilityCost::Tap)
                        && untapped_creatures.contains(&source_id)
                    {
                        return true;
                    }
                    ability.costs.iter().enumerate().any(|(cost_index, cost)| {
                        let AbilityCost::TapPermanents { constraint, .. } = cost else {
                            return false;
                        };
                        let Some(choice) = costs
                            .choices
                            .iter()
                            .find(|choice| choice.cost_index == cost_index as u32)
                        else {
                            return false;
                        };
                        let candidates: Vec<_> = choice
                            .candidate_ids
                            .iter()
                            .copied()
                            .filter(|candidate| {
                                !ability.costs.contains(&AbilityCost::Tap)
                                    || *candidate != source_id
                            })
                            .collect();
                        match *constraint {
                            ObjectPaymentConstraint::ExactCount(count) => {
                                candidates.len() >= count as usize
                                    && candidates
                                        .iter()
                                        .any(|candidate| untapped_creatures.contains(candidate))
                            }
                            ObjectPaymentConstraint::AggregateMinimum {
                                minimum,
                                contribution,
                            } => untapped_creatures
                                .iter()
                                .filter(|candidate| candidates.contains(candidate))
                                .any(|candidate| {
                                    let Some(candidate_value) =
                                        self.object_payment_contribution(*candidate, contribution)
                                    else {
                                        return false;
                                    };
                                    let other_max = candidates
                                        .iter()
                                        .copied()
                                        .filter(|other| other != candidate)
                                        .filter_map(|other| {
                                            self.object_payment_contribution(other, contribution)
                                        })
                                        .filter(|value| *value > 0)
                                        .try_fold(0_i64, |total, value| total.checked_add(value));
                                    other_max
                                        .and_then(|value| candidate_value.checked_add(value))
                                        .is_some_and(|total| total >= i64::from(minimum))
                                }),
                        }
                    })
                })
        })
    }

    fn activation_use_key(
        &self,
        permanent_id: ObjectId,
        identity: ActivationUseIdentity,
    ) -> ActivationUseKey {
        ActivationUseKey {
            object_id: permanent_id,
            zone_change_generation: self
                .state
                .zone_change_generation
                .get(&permanent_id)
                .copied()
                .unwrap_or(0),
            identity,
        }
    }

    fn persistent_activation_use_key(
        &self,
        permanent_id: ObjectId,
        occurrence: ActivatedAbilityOccurrence,
    ) -> PersistentActivationUseKey {
        PersistentActivationUseKey {
            object_id: permanent_id,
            zone_change_generation: self
                .state
                .zone_change_generation
                .get(&permanent_id)
                .copied()
                .unwrap_or(0),
            occurrence,
        }
    }

    fn activated_ability_occurrence(
        &self,
        permanent_id: ObjectId,
        ability_index: usize,
        ability: &ActivatedAbilityDef,
    ) -> Option<ActivatedAbilityOccurrence> {
        match ability.source_zone {
            AbilitySourceZone::Battlefield => self
                .effective_activated_abilities(permanent_id)
                .into_iter()
                .find(|effective| {
                    effective.slot as usize == ability_index
                        && effective.definition.ability_id == ability.ability_id
                })
                .map(|effective| effective.occurrence),
            AbilitySourceZone::Hand | AbilitySourceZone::Graveyard => {
                let (_, _, face_index) = self
                    .authored_zone_activated_abilities(permanent_id, ability.source_zone)
                    .into_iter()
                    .find(|(slot, candidate, _)| {
                        *slot == ability_index && candidate.ability_id == ability.ability_id
                    })?;
                Some(ActivatedAbilityOccurrence::Printed(
                    self.ability_definition(
                        permanent_id,
                        face_index,
                        vec![ability.ability_id.clone()],
                    ),
                ))
            }
        }
    }

    fn limited_activation_uses(
        &self,
        permanent_id: ObjectId,
        occurrence: &ActivatedAbilityOccurrence,
        ability: &ActivatedAbilityDef,
    ) -> Vec<LimitedActivationUse> {
        let mut uses = Vec::with_capacity(2);
        if ability.is_loyalty_ability() {
            uses.push(LimitedActivationUse::PerTurn(
                self.activation_use_key(permanent_id, ActivationUseIdentity::Loyalty),
            ));
        }
        if let Some(limit) = ability.activation_limit {
            uses.push(match limit {
                tricerules_card_model::primitives::ActivationLimit::PerTurn { .. } => {
                    LimitedActivationUse::PerTurn(self.activation_use_key(
                        permanent_id,
                        ActivationUseIdentity::Ability(occurrence.clone()),
                    ))
                }
                tricerules_card_model::primitives::ActivationLimit::PerObject { .. } => {
                    LimitedActivationUse::PerObject(
                        self.persistent_activation_use_key(permanent_id, occurrence.clone()),
                    )
                }
            });
        }
        uses
    }

    /// Oath changes the ordinary allowance, not the separate as-though permissions that other
    /// effects can create. Both source and planeswalker characteristics are evaluated live.
    fn ordinary_loyalty_activation_allowance(&self, permanent_id: ObjectId) -> u32 {
        let Some(object) = self.state.objects.get(&permanent_id) else {
            return 1;
        };
        if object.zone != Zone::Battlefield {
            return 1;
        }
        let Some(walker) = self
            .characteristics(permanent_id)
            .filter(|c| c.has_type("Planeswalker"))
        else {
            return 1;
        };
        let has_permission = self.state.objects.values().any(|source| {
            source.zone == Zone::Battlefield
                && self.controller_of(source.id) == Some(walker.controller)
                && self
                    .active_static_ability_definitions(source.id)
                    .iter()
                    .any(|ability| {
                        matches!(
                            ability,
                            StaticAbilityDef::ControllerMayActivatePlaneswalkerLoyaltyTwicePerTurn
                        )
                    })
        });
        if has_permission {
            2
        } else {
            1
        }
    }

    fn loyalty_activation_authorization(
        &self,
        permanent_id: ObjectId,
        player: PlayerId,
    ) -> Option<LoyaltyActivationAuthorization> {
        let uses = self
            .state
            .activation_uses_this_turn
            .get(&self.activation_use_key(permanent_id, ActivationUseIdentity::Loyalty))
            .copied()
            .unwrap_or(0);
        if uses < self.ordinary_loyalty_activation_allowance(permanent_id) {
            return Some(LoyaltyActivationAuthorization::Ordinary);
        }
        let object = self.state.objects.get(&permanent_id)?;
        if object.zone != Zone::Battlefield {
            return None;
        }
        let characteristics = self.characteristics(permanent_id)?;
        if characteristics.controller != player || !characteristics.has_type("Planeswalker") {
            return None;
        }
        let generation = self.payment_object_ref(permanent_id).zone_change_generation;
        self.state
            .chain_veil_permissions
            .iter()
            .find(|permission| {
                permission.player == player
                    && !permission
                        .used_walkers
                        .contains(&(permanent_id, generation))
            })
            .map(|permission| LoyaltyActivationAuthorization::ChainVeil {
                permission_id: permission.id,
                walker: permanent_id,
                generation,
            })
    }

    fn activation_limit_allows(
        &self,
        permanent_id: ObjectId,
        ability_index: usize,
        ability: &ActivatedAbilityDef,
    ) -> bool {
        if ability.is_loyalty_ability()
            && self
                .controller_of(permanent_id)
                .and_then(|player| self.loyalty_activation_authorization(permanent_id, player))
                .is_none()
        {
            return false;
        }
        let Some(limit) = ability.activation_limit else {
            return true;
        };
        let Some(occurrence) =
            self.activated_ability_occurrence(permanent_id, ability_index, ability)
        else {
            return false;
        };
        match limit {
            tricerules_card_model::primitives::ActivationLimit::PerTurn { max_activations } => {
                self.state
                    .activation_uses_this_turn
                    .get(&self.activation_use_key(
                        permanent_id,
                        ActivationUseIdentity::Ability(occurrence),
                    ))
                    .copied()
                    .unwrap_or(0)
                    < max_activations
            }
            tricerules_card_model::primitives::ActivationLimit::PerObject { max_activations } => {
                self.state
                    .activation_uses_per_object
                    .get(&self.persistent_activation_use_key(permanent_id, occurrence))
                    .copied()
                    .unwrap_or(0)
                    < max_activations
            }
        }
    }

    pub(super) fn record_limited_activations(&mut self, uses: Vec<LimitedActivationUse>) {
        for usage in uses {
            let count = match usage {
                LimitedActivationUse::PerTurn(key) => {
                    self.state.activation_uses_this_turn.entry(key).or_insert(0)
                }
                LimitedActivationUse::PerObject(key) => self
                    .state
                    .activation_uses_per_object
                    .entry(key)
                    .or_insert(0),
            };
            *count = count.saturating_add(1);
        }
    }

    fn activation_conditions_hold(
        &self,
        permanent_id: ObjectId,
        ability: &tricerules_card_model::ActivatedAbilityDef,
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
        ability.conditions.iter().all(|condition| {
            self.condition_holds(
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
                    stack_item: None,
                    previous_effect_result: None,
                },
            )
        })
    }

    pub(super) fn check_tappable(
        &self,
        permanent_id: ObjectId,
        _card_id: &str,
    ) -> Result<(), EngineError> {
        let has_haste = self
            .characteristics(permanent_id)
            .is_some_and(|value| value.has_keyword(tricerules_card_model::Keyword::Haste));
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
        card_id: &str,
        effective: &EffectiveActivatedAbility,
        mana_option_index: u32,
        targets: &[rv1::TargetRef],
        flex_payments: &[rv1::FlexPipPayment],
        cost_selections: &[rv1::CostSelection],
        restricted_mana: &[rv1::ManaSpendSelection],
        x_value: u32,
        mana_split_first_color_count: u32,
        selection: Option<&rv1::PaymentSelection>,
    ) -> Result<RuledEventBatch, EngineError> {
        let attack_payment_before = self
            .state
            .pending_attack_declaration
            .as_ref()
            .filter(|pending| pending.attacking_player_id == player)
            .map(|_| self.state.clone());
        let ability = &effective.definition;
        let ability_path = effective.ability_path();
        let ability_path = ability_path.as_slice();
        if !targets.is_empty() {
            return Err(EngineError::Illegal("mana ability takes no targets"));
        }
        let concealed = self.state.objects[&permanent_id].face_down;
        let card_name = if concealed {
            "Face-down permanent".to_string()
        } else {
            self.registry
                .get(card_id)
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| card_id.to_owned())
        };
        let mana_damage = ability.mana_ability_damage_to_controller().map(|amount| {
            self.prepare_mana_ability_damage(permanent_id, player, card_name.clone(), amount)
        });
        let attack_mana_damage = attack_payment_before
            .as_ref()
            .and_then(|_| mana_damage.clone());
        self.validate_activation_mana_choice(
            permanent_id,
            AbilitySourceZone::Battlefield,
            ability,
            mana_option_index,
            x_value,
            mana_split_first_color_count,
        )?;
        let amount = if let Some((first_color, second_color)) = ability.storage_counter_split_mana()
        {
            let mut amount = mana_amount_for_color(first_color, mana_split_first_color_count);
            let second_amount =
                mana_amount_for_color(second_color, x_value - mana_split_first_color_count);
            amount.w += second_amount.w;
            amount.u += second_amount.u;
            amount.b += second_amount.b;
            amount.r += second_amount.r;
            amount.g += second_amount.g;
            amount.c += second_amount.c;
            let multiplier = self.tapped_permanent_mana_multiplier(permanent_id, ability);
            multiply_mana_amount(amount, multiplier)
        } else {
            let Some(options) = self.active_mana_options(permanent_id, ability) else {
                return Err(EngineError::Illegal("not a mana ability"));
            };
            options
                .get(mana_option_index as usize)
                .copied()
                .ok_or(EngineError::Illegal("invalid mana option"))?
        };
        let activation_uses =
            self.limited_activation_uses(permanent_id, &effective.occurrence, ability);

        let primary_presentation = if concealed || ability.intrinsic_land_mana {
            None
        } else {
            let face_name = self
                .effective_face(permanent_id)
                .map(|face| face.name.clone())
                .unwrap_or_else(|| card_id.to_owned());
            effective
                .presentation_definition
                .as_ref()
                .map(|definition| {
                    ability_presentation(
                        self.registry,
                        definition,
                        &ability.presentation,
                        ability.fallback_text_with_path(&face_name, ability_path),
                    )
                })
        };

        let mana_restriction = ability.mana_restriction().cloned();
        let mana_spending_origin = mana_restriction
            .as_ref()
            .filter(|restriction| {
                super::mana_spending::restriction_requires_source_tracking(restriction)
            })
            .map(|_| {
                let source_face_index = self
                    .state
                    .objects
                    .get(&permanent_id)
                    .map(|object| object.face_up_index)
                    .unwrap_or(0);
                crate::state::ManaSpendingOrigin {
                    source_object_id: permanent_id,
                    source_zone_change_generation: self
                        .state
                        .zone_change_generation
                        .get(&permanent_id)
                        .copied()
                        .unwrap_or(0),
                    source_face_change_generation: self
                        .state
                        .face_change_generation
                        .get(&permanent_id)
                        .copied()
                        .unwrap_or(0),
                    source_face_index,
                    controller: player,
                    card_id: card_id.to_owned(),
                    source_label: card_name.clone(),
                    ability_presentation: primary_presentation.clone(),
                }
            });

        let prepared = self.prepare_ability_costs(
            player,
            idx,
            permanent_id,
            &ability.costs,
            flex_payments,
            cost_selections,
            restricted_mana,
            x_value,
            0,
            self.activated_mana_reduction(player, permanent_id, ability)?,
        )?;
        let cost_plan = self.finish_ability_payment(player, permanent_id, prepared, selection)?;
        let attack_cost_plan = attack_payment_before.as_ref().map(|_| cost_plan.clone());
        let payment = self.commit_cost_transaction(cost_plan)?;
        let payment_receipt = payment.clone();
        self.record_limited_activations(activation_uses.clone());

        let restriction_presentation = mana_restriction.as_ref().and_then(|restriction| {
            primary_presentation.as_ref().map(|parent| {
                child_presentation_ref(
                    parent,
                    PresentationPath::ManaRestriction(&restriction.restriction_id),
                    &restriction.presentation,
                    restriction.fallback_label(),
                )
            })
        });
        let restriction_group_id = if let Some(restriction) = mana_restriction.as_ref() {
            Some(self.register_mana_restriction_group(
                restriction,
                restriction_presentation,
                mana_spending_origin.clone(),
                None,
            )?)
        } else {
            None
        };
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

        let cost_triggers =
            self.collect_committed_cost_triggers(payment.trigger_events, payment.sacrificed);
        let has_cost_triggers = !cost_triggers.is_empty();
        if mana_damage.is_none()
            && !has_cost_triggers
            && matches!(ability.costs.as_slice(), [AbilityCost::Tap])
        {
            self.state
                .undoable_mana_abilities
                .push(UndoableManaAbility {
                    player,
                    source: permanent_id,
                    produced: amount,
                    restriction_group_id,
                });
        }

        let ability_text = ability.fallback_text_with_path(&card_name, ability_path);
        let paid_costs_line = format_paid_card_costs_log(&payment.paid_card_costs);

        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log_ability(
            format!("P{player} activates {card_name}{paid_costs_line}: "),
            &ability_text,
            primary_presentation,
            String::new(),
        ));
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
                "P{player} pays {} life.",
                payment.life_paid
            )));
        }
        // A mana ability does not use the stack (CR 605.3a). Cost triggers are staged before its
        // immediate effect; composite damage also invalidates any earlier float undo.
        let event_time_cost_triggers = cost_triggers.clone();
        self.stage_triggers(cost_triggers);
        let mut attack_mana_damage_results = Vec::new();
        let mut attack_mana_damage_triggers = Vec::new();
        if let Some(damage) = mana_damage {
            self.state.undoable_mana_abilities.clear();
            (attack_mana_damage_results, attack_mana_damage_triggers) =
                self.resolve_mana_ability_damage(damage, &mut batch.events)?;
        }
        if let Some(before_state) = attack_payment_before {
            self.record_attack_mana_ability(super::combat::AttackManaAbilityReceiptInput {
                before_state,
                source: permanent_id,
                ability_path: effective.ability_path(),
                source_label: card_name,
                ability: ability.clone(),
                cost_plan: attack_cost_plan.expect("pending attack payment captured its cost plan"),
                payment: payment_receipt,
                mana_output: amount,
                restriction_group_id,
                activation_uses,
                mana_damage: attack_mana_damage,
                mana_damage_results: attack_mana_damage_results,
                mana_damage_triggers: attack_mana_damage_triggers,
                cost_triggers: event_time_cost_triggers,
                has_cost_triggers,
                mana_spending_origin,
            });
        }
        Ok(batch)
    }

    pub(super) fn undo_mana_ability(
        &mut self,
        player: PlayerId,
        command: &rv1::UndoManaAbility,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.pending_attack_declaration.is_some() || command.attack_transaction_id != 0 {
            return self.undo_attack_mana_ability(player, command);
        }
        let payment_undo_start = self.state.pending_resolution.as_ref().and_then(|pending| {
            (pending.deciding_player == player)
                .then_some(pending.continuation.mana_window_undo_start()?)
        });
        let spell_payment = self
            .state
            .pending_spell_cast
            .as_ref()
            .is_some_and(|pending| pending.caster == player)
            || self.paying_ability_activation(player);
        if self.state.priority_player_id() != player
            && payment_undo_start.is_none()
            && !spell_payment
        {
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
        let entry = self.state.undoable_mana_abilities[pos].clone();

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
            let player_state = &mut self.state.players[idx];
            let retained = player_state.retained_combat_mana;
            let pool = &mut player_state.mana_pool;
            if pool.white.saturating_sub(retained.white) < p.w
                || pool.blue.saturating_sub(retained.blue) < p.u
                || pool.black.saturating_sub(retained.black) < p.b
                || pool.red.saturating_sub(retained.red) < p.r
                || pool.green.saturating_sub(retained.green) < p.g
                || pool.colorless.saturating_sub(retained.colorless) < p.c
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

        self.state.undoable_mana_abilities.remove(pos);

        super::set_tapped(&mut self.state, entry.source, false);

        let card_name = self
            .state
            .objects
            .get(&entry.source)
            .map(|o| {
                if o.face_down {
                    "Face-down permanent".to_string()
                } else {
                    self.registry
                        .get(&o.card_id)
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| "permanent".to_string())
                }
            })
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
        let special_permission = self
            .special_cast_permission(player)
            .filter(|permission| permission.scope == ExilePlayPermissionScope::PlayCard);
        let special_resume_stack = special_permission.as_ref().and_then(|_| {
            self.state
                .pending_resolution
                .as_ref()
                .and_then(|pending| pending.continuation.stack())
                .cloned()
        });
        let special = special_permission.is_some();
        if self.state.priority_player_id() != player && !special {
            return Err(EngineError::Illegal("not your priority"));
        }
        let max_lands = 1 + self.extra_land_plays_for(player);
        if self.state.lands_played_this_turn >= max_lands {
            return Err(EngineError::Illegal(
                "land play limit reached for this turn",
            ));
        }
        if special && self.state.active_player_id() != player {
            return Err(EngineError::Illegal(
                "resolution-time land play requires the player's turn",
            ));
        }
        if !special && !super::priority::sorcery_speed_available(&self.state, player) {
            return Err(EngineError::Illegal("play land only at sorcery speed"));
        }
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let source = command
            .source
            .as_ref()
            .ok_or(EngineError::Illegal("missing land source"))?;
        let location = source
            .location
            .as_ref()
            .ok_or(EngineError::Illegal("missing land source"))?;
        let (oid, require_graveyard_land_card) = match location {
            rv1::land_source::Location::HandIndex(hand_index) => {
                if special {
                    return Err(EngineError::Illegal(
                        "resolution-time land play must use the selected exile object",
                    ));
                }
                (
                    *self.state.players[idx]
                        .hand
                        .get(*hand_index as usize)
                        .ok_or(EngineError::Illegal("bad hand index"))?,
                    false,
                )
            }
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
                if source.expected_zone_change_generation != Some(generation) {
                    return Err(EngineError::Illegal("stale exile land source"));
                }
                let permitted = if let Some(permission) = special_permission.as_ref() {
                    permission.object_id == *object_id
                        && permission.zone_change_generation == generation
                        && permission.scope == ExilePlayPermissionScope::PlayCard
                } else {
                    self.state
                        .active_exile_play_permissions
                        .iter()
                        .any(|permission| {
                            permission.player_id == player
                                && permission.object_id == *object_id
                                && permission.zone_change_generation == generation
                                && permission.scope == ExilePlayPermissionScope::PlayCard
                        })
                };
                if object.zone != Zone::Exile || !permitted {
                    return Err(EngineError::Illegal("illegal land play from exile"));
                }
                (*object_id, false)
            }
            rv1::land_source::Location::GraveyardObjectId(object_id) => {
                if special {
                    return Err(EngineError::Illegal(
                        "resolution-time land play must use the selected exile object",
                    ));
                }
                let object = self
                    .state
                    .objects
                    .get(object_id)
                    .ok_or(EngineError::Illegal("unknown graveyard object"))?;
                let generation = self
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0);
                if source.expected_zone_change_generation != Some(generation) {
                    return Err(EngineError::Illegal("stale graveyard land source"));
                }
                if object.zone != Zone::Graveyard
                    || object.owner != player
                    || !self.state.players[idx].graveyard.contains(object_id)
                    || !self.can_play_lands_from_own_graveyard(player)
                {
                    return Err(EngineError::Illegal("illegal land play from graveyard"));
                }
                (*object_id, true)
            }
            rv1::land_source::Location::LibraryObjectId(object_id) => {
                if special {
                    return Err(EngineError::Illegal(
                        "resolution-time land play must use the selected exile object",
                    ));
                }
                let object = self
                    .state
                    .objects
                    .get(object_id)
                    .ok_or(EngineError::Illegal("unknown library object"))?;
                let generation = self
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0);
                if source.expected_zone_change_generation != Some(generation) {
                    return Err(EngineError::Illegal("stale library land source"));
                }
                if object.zone != Zone::Library
                    || object.owner != player
                    || self.state.players[idx].library.front() != Some(object_id)
                    || !self.may_play_library_top_land(player)
                {
                    return Err(EngineError::Illegal("illegal land play from library"));
                }
                (*object_id, false)
            }
        };
        let card_id = self.state.objects.get(&oid).unwrap().card_id.clone();
        let def = self
            .registry
            .get(&card_id)
            .ok_or_else(|| EngineError::MissingCard(card_id.clone()))?;
        if require_graveyard_land_card && !def.matches_card_type_outside_stack(CardTypeFilter::Land)
        {
            return Err(EngineError::Illegal("not a land card in graveyard"));
        }
        // CR 712: public-zone land plays use the same face-choice rule as hand land plays.
        let face = def
            .face(face_index)
            .ok_or(EngineError::Illegal("bad face index"))?;
        if !def.face_available_from_hand(face_index) {
            return Err(EngineError::Illegal("face cannot be played as a land"));
        }
        if !face.is_land {
            return Err(EngineError::Illegal("not a land"));
        }
        let land_name = if def.is_multiface() {
            face.name.to_string()
        } else {
            def.name.clone()
        };
        let face_down_exile_reveal_events = super::reveals::face_down_exile_reveal_events(
            &self.state,
            self.registry,
            oid,
            "playing a card from exile",
        )
        .unwrap_or_default();
        let revealed_face_down_exile = !face_down_exile_reveal_events.is_empty();
        let face_down_exile_original_face_up_index = revealed_face_down_exile.then(|| {
            self.state
                .objects
                .get(&oid)
                .expect("validated exile land remains in the game")
                .face_up_index
        });
        if revealed_face_down_exile {
            let object = self
                .state
                .objects
                .get_mut(&oid)
                .expect("validated exile land remains in the game");
            object.face_down = false;
            object.face_up_index = face_index;
        }
        self.state.lands_played_this_turn += 1;
        let mut batch = RuledEventBatch::default();
        batch.events.extend(face_down_exile_reveal_events);
        let item = StackItem {
            mana_colors_spent_to_cast: Default::default(),
            id: oid,
            controller: player,
            card_id,
            prototyped: false,
            targets: Vec::new(),
            ability_text: Some("land play".to_string()),
            source_permanent_id: None,
            source_owner: None,
            source_zone_change: self
                .state
                .zone_change_generation
                .get(&oid)
                .copied()
                .unwrap_or(0),
            trigger_use_key: None,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            activated_ability_occurrence: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index,
            cast_method: SpellCastMethod::Normal,
            returned_attacker_assignment: None,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: None,
            cast_cost_receipts: Vec::new(),
            payment_result: CardResultCohort::default(),
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: TriggerContext::default(),
        };
        if special {
            self.state.pending_resolution = None;
        }
        let entry_progress = match self.begin_battlefield_entry(
            item,
            BattlefieldEntryEvent {
                entry_reveal_receipts: Vec::new(),
                mana_colors_spent_to_cast: Default::default(),
                prepared: false,
                object_id: oid,
                deciding_player: player,
                destination_controller: player,
                battle_protector: None,
                face_index,
                unlock_room_door: None,
                chosen_x: 0,
                cast_by: None,
                cast_cost_receipts: Vec::new(),
                player_life_snapshot: self.player_life_snapshot(),
                tapped: false,
                set_types: None,
                chosen_basic_land_type: None,
                chosen_opponents: Vec::new(),
                chosen_creature_types: Vec::new(),
                entry_counters: BTreeMap::new(),
                entry_counter_placements: Vec::new(),
                entry_counter_replacements_resolved: false,
                entry_modifiers: Vec::new(),
                entry_cause: None,
                attached_to: None,
                pending_copy_candidate: None,
                pending_aura_recipient: None,
                accepted_aura_recipient: None,
                applied_effects: Vec::new(),
            },
            BattlefieldEntryCompletion::LandPlay {
                player,
                land_name: land_name.clone(),
                resume_stack: special_resume_stack.clone().map(Box::new),
            },
            &mut batch.events,
        ) {
            Ok(progress) => progress,
            Err(error) => {
                // A refused entry cannot publish the queued reveal receipt. Restore the hidden
                // incarnation as well, guarded by the generation validated above.
                if revealed_face_down_exile
                    && self.state.zone_change_generation.get(&oid).copied()
                        == Some(source.expected_zone_change_generation.unwrap_or_default())
                {
                    if let Some(object) = self
                        .state
                        .objects
                        .get_mut(&oid)
                        .filter(|object| object.zone == Zone::Exile)
                    {
                        object.face_down = true;
                        if let Some(face_up_index) = face_down_exile_original_face_up_index {
                            object.face_up_index = face_up_index;
                        }
                    }
                }
                return Err(error);
            }
        };
        match entry_progress {
            super::replacement::BattlefieldEntryProgress::Parked => {
                if let Some(stack) = special_resume_stack.as_ref() {
                    self.transfer_entry_choice_resume(stack);
                }
                return Ok(batch);
            }
            super::replacement::BattlefieldEntryProgress::Skipped(entry) => {
                self.restore_skipped_battlefield_entry(&entry)?;
            }
            super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                let entry = *entry;
                self.commit_battlefield_entry(entry, None, &mut batch.events)?;
                batch.events.push(permanent_moved_event(
                    &self.state,
                    oid,
                    player,
                    rv1::permanent_moved::Destination::Battlefield,
                ));
            }
        }
        batch
            .events
            .push(ev_log(format!("P{player} played {land_name}")));
        if special {
            self.state.passes_since_stack_change = 0;
            let resume_stack = special_resume_stack;
            return if let Some(stack) = resume_stack {
                self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    stack.previous_result,
                    batch.events,
                )
            } else {
                Ok(finish_with_events(self, batch.events))
            };
        }
        self.state.passes_since_stack_change = 0;
        fill_legal(&mut batch, self);
        Ok(batch)
    }
}

#[cfg(test)]
mod cast_snapshot_tests {
    use super::*;

    #[test]
    fn prototype_is_offered_as_a_choice_separate_from_cast_method() {
        let mut e = engine_with_extra(
            "",
            &[
                r#"(id: "prototype_test", name: "Prototype Test", face_id: "prototype_test",
                mana_cost: "{9}", types: ["Artifact", "Creature"], power: 4, toughness: 4,
                prototype: Some((mana_cost: "{3}{R}{R}", power: 2, toughness: 2)))"#,
            ],
        );
        let oid = add(&mut e, 0, "prototype_test", Zone::Hand);
        let slot = e.state.players[0]
            .hand
            .iter()
            .position(|id| *id == oid)
            .expect("prototype card in hand") as u32;
        let offers = e.initial_response_batch().legal_by_player[&0]
            .hand_actions
            .iter()
            .filter(|action| {
                action.hand_index == slot
                    && action.kind == rv1::HandActionKind::HandActionCastSpell as i32
            })
            .map(|action| (action.cast_method, action.prototyped, action.cost.clone()))
            .collect::<Vec<_>>();

        assert_eq!(
            offers,
            [
                (rv1::CastMethod::Normal as i32, false, "{9}".into()),
                (rv1::CastMethod::Normal as i32, true, "{3}{R}{R}".into()),
            ]
        );

        e.state.players[0].mana_pool.red = 2;
        e.state.players[0].mana_pool.colorless = 3;
        let mut cast = command(&e, oid);
        let Some(rv1::ruled_command::Cmd::CastSpell(cast_spell)) = cast.cmd.as_mut() else {
            unreachable!()
        };
        cast_spell.prototyped = true;
        e.apply_scenario_command(0, &cast)
            .expect("the inset cost casts the Prototype mode");
        let stack_item = e.state.stack.last().expect("spell on stack");
        assert!(stack_item.prototyped);
        let characteristics = e.characteristics(oid).expect("stack characteristics");
        assert_eq!(characteristics.mana_value, 5);
        assert_eq!(characteristics.colors, vec![Color::Red]);
        assert_eq!(
            (characteristics.power, characteristics.toughness),
            (Some(2), Some(2))
        );

        resolve(&mut e);
        assert_eq!(e.state.objects[&oid].zone, Zone::Battlefield);
        let characteristics = e.characteristics(oid).expect("battlefield characteristics");
        assert_eq!(characteristics.mana_value, 5);
        assert_eq!(
            (characteristics.power, characteristics.toughness),
            (Some(2), Some(2))
        );
    }

    fn legendary_cast_fixture() -> GameEngine {
        engine_with_extra(
            "spell_effect: [GainLife(amount: 1)]",
            &[
                r#"(id: "legendary_spell", name: "Legendary Spell", face_id: "legendary_spell", mana_cost: "{B}", types: ["Sorcery"], supertypes: ["Legendary"], spell_effect: [GainLife(amount: 1)])"#,
                r#"(id: "legendary_creature", name: "Legendary Creature", face_id: "legendary_creature", types: ["Creature"], supertypes: ["Legendary"], power: 2, toughness: 2)"#,
                r#"(id: "legendary_walker", name: "Legendary Walker", face_id: "legendary_walker", types: ["Planeswalker"], supertypes: ["Legendary"], loyalty: 4)"#,
            ],
        )
    }

    #[test]
    fn legendary_spell_qualification_uses_derived_types_and_control_not_printed_values() {
        let mut e = legendary_cast_fixture();
        let qualifier = add(&mut e, 1, "legendary_creature", Zone::Battlefield);
        let face = e.registry.get("legendary_spell").unwrap().primary_face();
        assert!(!e.legendary_spell_cast_allowed(0, face));
        e.state.continuous_effects.push(ContinuousEffect {
            source_id: None,
            trigger_grant_origin: None,
            affected: AffectedScope::Single(qualifier),
            kind: ContinuousEffectKind::Layer2Control {
                controller: tricerules_card_model::ControllerReference::Fixed(0),
            },
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: 1,
        });
        assert!(e.legendary_spell_cast_allowed(0, face));
        e.state.continuous_effects.push(ContinuousEffect {
            source_id: None,
            trigger_grant_origin: None,
            affected: AffectedScope::Single(qualifier),
            kind: ContinuousEffectKind::Layer4SetTypeLine(
                tricerules_card_model::TypeLineReplacement {
                    card_types: vec![tricerules_card_model::PermanentTypeFilter::Artifact],
                    creature_types: vec![],
                    land_types: vec![],
                },
            ),
            condition: None,
            duration: EffectDuration::Indefinite,
            timestamp: 2,
        });
        assert!(!e.legendary_spell_cast_allowed(0, face));
        assert!(
            e.registry
                .get("legendary_creature")
                .unwrap()
                .primary_face()
                .is_creature
        );
    }

    #[test]
    fn legendary_spell_special_resolution_cast_permission_cannot_bypass_restriction() {
        let mut e = engine_with_extra(
            "spell_effect: [GainLife(amount: 1)]",
            &[
                r#"(id: "legendary_siege_reverse", name: "Legendary Siege // Reverse", layout: Transform, faces: [
                (name: "Legendary Siege", face_id: "legendary_siege", types: ["Battle", "Siege"], defense: 3),
                (name: "Reverse", face_id: "reverse", types: ["Sorcery"], supertypes: ["Legendary"], spell_effect: [GainLife(amount: 1)]),
            ])"#,
                r#"(id: "special_cast_qualifier", name: "Special Cast Qualifier", face_id: "special_cast_qualifier", types: ["Creature"], supertypes: ["Legendary"], power: 2, toughness: 2)"#,
            ],
        );
        let siege = add(&mut e, 0, "legendary_siege_reverse", Zone::Battlefield);
        e.state.objects.get_mut(&siege).unwrap().counters.clear();
        e.stage_siege_defeat_trigger(siege);
        e.flush_staged_triggers(&mut vec![]);
        let mut initial_offer = None;
        while e.state.pending_resolution.is_none() {
            let batch = e
                .apply_scenario_command(
                    e.state.priority_player_id(),
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                    },
                )
                .unwrap();
            for event in batch.events {
                if let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) = event.ev {
                    initial_offer = Some(choice);
                }
            }
        }
        let special_choice = |event: rv1::RuledEvent| {
            let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) = event.ev else {
                panic!("special cast prompt");
            };
            choice
        };
        assert_eq!(
            special_choice(e.resolution_payment_choice_event().unwrap()).candidate_selectable,
            vec![false]
        );
        assert_eq!(initial_offer.unwrap().candidate_selectable, vec![false]);
        let permission = e
            .special_cast_permission(0)
            .expect("special resolution permission");
        let cast = rv1::CastSpell {
            cast_method: rv1::CastMethod::SiegeDefeat as i32,
            casting_permission_id: Some(permission.group_id),
            source: Some(rv1::CastSource {
                expected_zone_change_generation: Some(e.state.zone_change_generation[&siege]),
                location: Some(rv1::cast_source::Location::ExileObjectId(siege)),
            }),
            face_index: 1,
            ..Default::default()
        };
        let before = e.diagnostic_snapshot().unwrap();
        let result = e.apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                        cast_spell: Some(cast),
                        ..Default::default()
                    },
                )),
            },
        );
        assert!(matches!(result, Err(EngineError::Illegal(_))));
        assert_eq!(e.diagnostic_snapshot().unwrap(), before);
        assert!(e.initial_response_batch().legal_by_player[&0]
            .zone_cast_actions
            .is_empty());
        let qualifier = add(&mut e, 0, "special_cast_qualifier", Zone::Battlefield);
        let refreshed = special_choice(e.resolution_payment_choice_event().unwrap());
        assert_eq!(refreshed.candidate_selectable, vec![true]);
        assert_eq!(refreshed.candidate_names, vec!["Reverse"]);
        assert_eq!(
            e.initial_response_batch().legal_by_player[&0]
                .zone_cast_actions
                .len(),
            1
        );
        move_object_to_zone(&mut e.state, e.registry, qualifier, Zone::Graveyard, None).unwrap();
        assert_eq!(
            special_choice(e.resolution_payment_choice_event().unwrap()).candidate_selectable,
            vec![false]
        );
        e.apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        decision: rv1::ResolutionChoiceDecision::Decline as i32,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
        assert!(e.state.pending_resolution.is_none());
        assert!(e.state.stack.is_empty());
        assert_eq!(e.state.objects[&siege].zone, Zone::Exile);
    }

    #[test]
    fn all_graveyard_return_printed_permanent_guard_includes_battle_and_deduplicates_or_overlap() {
        let cards = [
            r#"(id: "return_land", name: "Return Land", face_id: "return_land", types: ["Land"], supertypes: ["Legendary"])"#,
            r#"(id: "return_battle", name: "Return Battle", face_id: "return_battle", types: ["Battle"], supertypes: ["Legendary"], defense: 3)"#,
            r#"(id: "return_overlap", name: "Return Overlap", face_id: "return_overlap", types: ["Artifact", "Enchantment"], supertypes: ["Legendary"])"#,
            r#"(id: "return_instant", name: "Return Instant", face_id: "return_instant", types: ["Instant"], supertypes: ["Legendary"], spell_effect: [GainLife(amount: 1)])"#,
        ];
        for filter in [
            r#"(required_supertypes: ["Legendary"])"#,
            r#"(any_of: Some([(card_type: Some(Artifact)), (card_type: Some(Enchantment))]))"#,
        ] {
            let mut e = engine_with_extra(
                &format!("spell_effect: [ReturnAllGraveyardPermanents(players: Controller, filter: {filter})]"),
                &cards,
            );
            let ids: Vec<_> = [
                "return_land",
                "return_battle",
                "return_overlap",
                "return_instant",
            ]
            .iter()
            .map(|card| add(&mut e, 0, card, Zone::Graveyard))
            .collect();
            let opponent = add(&mut e, 1, "return_overlap", Zone::Graveyard);
            let spell = add(&mut e, 0, "snapshot_spell", Zone::Hand);
            e.state.players[0].mana_pool.black = 1;
            e.apply_scenario_command(0, &command(&e, spell)).unwrap();
            resolve(&mut e);
            for _ in 0..4 {
                let Some(pending) = e.state.pending_resolution.clone() else {
                    break;
                };
                assert!(matches!(
                    pending.presentation.choice_kind,
                    rv1::ChoiceKind::BattleProtector | rv1::ChoiceKind::SimultaneousEntryOrder
                ));
                assert!(ids
                    .iter()
                    .all(|oid| e.state.objects[oid].zone == Zone::Graveyard));
                e.apply_scenario_command(
                    0,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                            rv1::SubmitResolutionChoice {
                                chosen_object_ids: if pending.presentation.choice_kind
                                    == rv1::ChoiceKind::BattleProtector
                                {
                                    vec![pending.presentation.candidates[0]]
                                } else {
                                    pending.presentation.candidates
                                },
                                ..Default::default()
                            },
                        )),
                    },
                )
                .unwrap();
            }
            let legendary = filter.contains("Legendary");
            for (index, oid) in ids.iter().enumerate() {
                let enters = index == 2 || (legendary && index < 2);
                assert_eq!(
                    e.state.objects[oid].zone,
                    if enters {
                        Zone::Battlefield
                    } else {
                        Zone::Graveyard
                    }
                );
                assert_eq!(
                    e.state
                        .zone_change_generation
                        .get(oid)
                        .copied()
                        .unwrap_or(0),
                    u64::from(enters)
                );
            }
            assert_eq!(
                e.state.players[0]
                    .battlefield
                    .iter()
                    .filter(|oid| **oid == ids[2])
                    .count(),
                1
            );
            assert_eq!(e.state.objects[&opponent].zone, Zone::Graveyard);
            if legendary {
                assert_eq!(
                    e.state.objects[&ids[1]].counter_count(CounterKind::Defense),
                    3
                );
            }
            assert!(e.state.stack.is_empty());
        }
    }

    #[test]
    fn legendary_spell_uses_current_controller_and_commits_after_qualifier_leaves() {
        for card in ["legendary_creature", "legendary_walker"] {
            let mut e = legendary_cast_fixture();
            // Opponent owns the qualifier; caster controls it. Ownership is irrelevant.
            let qualifier = add(&mut e, 1, card, Zone::Battlefield);
            e.state.objects.get_mut(&qualifier).unwrap().base_controller = 0;
            let spell = add(&mut e, 0, "legendary_spell", Zone::Hand);
            e.state.players[0].mana_pool.black = 1;
            assert!(e.initial_response_batch().legal_by_player[&0]
                .hand_actions
                .iter()
                .any(|action| action.hand_index as usize == e.state.players[0].hand.len() - 1));
            let cast = command(&e, spell);
            let Some(rv1::ruled_command::Cmd::CastSpell(cast)) = cast.cmd else {
                unreachable!()
            };
            let begin = RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::BeginSpellCast(
                    rv1::BeginSpellCast {
                        announcement: Some(announcement_from_cast(&cast)),
                    },
                )),
            };
            e.apply_scenario_command(0, &begin)
                .expect("controlled legendary qualifier allows Begin");
            let transaction_id = e.state.pending_spell_cast.as_ref().unwrap().transaction_id;
            // Model its loss while paying costs after the lawful announcement.
            super::super::resolution::move_object_to_zone(
                &mut e.state,
                e.registry,
                qualifier,
                Zone::Graveyard,
                None,
            )
            .unwrap();
            assert!(!e.legendary_spell_cast_allowed(
                0,
                e.registry.get("legendary_spell").unwrap().primary_face()
            ));
            e.apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
                        rv1::CommitSpellCast {
                            transaction_id,
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("commit does not recheck the announced casting restriction");
            assert_eq!(e.state.objects[&spell].zone, Zone::Stack);
            assert_eq!(e.state.players[0].mana_pool.black, 0);
            resolve(&mut e);
            assert_eq!(e.state.players[0].life, 21);
        }
    }

    #[test]
    fn legendary_spell_requires_own_legendary_creature_or_planeswalker_before_begin() {
        for spell_type in ["Instant", "Sorcery"] {
            for qualifier in [
                None,
                Some((0, "legendary_artifact")),
                Some((1, "legendary_creature")),
            ] {
                let definition = format!(
                    r#"(id: "legendary_spell", name: "Legendary Spell", face_id: "legendary_spell", mana_cost: "{{B}}", types: ["{spell_type}"], supertypes: ["Legendary"], spell_effect: [GainLife(amount: 1)])"#
                );
                let mut e = engine_with_extra(
                    "spell_effect: [GainLife(amount: 1)]",
                    &[
                        &definition,
                        r#"(id: "legendary_artifact", name: "Legendary Artifact", face_id: "legendary_artifact", types: ["Artifact"], supertypes: ["Legendary"])"#,
                        r#"(id: "legendary_creature", name: "Legendary Creature", face_id: "legendary_creature", types: ["Creature"], supertypes: ["Legendary"], power: 2, toughness: 2)"#,
                    ],
                );
                if let Some((player, card)) = qualifier {
                    add(&mut e, player, card, Zone::Battlefield);
                }
                let spell = add(&mut e, 0, "legendary_spell", Zone::Hand);
                e.state.players[0].mana_pool.black = 1;
                let before_index = e.state.command_index;
                let before_hand = e.state.players[0].hand.clone();
                let cast = command(&e, spell);
                assert!(
                    matches!(
                        e.apply_scenario_command(0, &cast),
                        Err(EngineError::Illegal(_))
                    ),
                    "{spell_type} with {qualifier:?} must be rejected"
                );
                assert_eq!(e.state.command_index, before_index);
                assert_eq!(e.state.players[0].hand, before_hand);
                assert_eq!(e.state.objects[&spell].zone, Zone::Hand);
                assert_eq!(e.state.players[0].mana_pool.black, 1);
                assert!(e.state.pending_spell_cast.is_none());
                assert!(e.state.stack.is_empty());
                assert!(
                    !e.initial_response_batch().legal_by_player[&0]
                        .hand_actions
                        .iter()
                        .any(|action| action.hand_index as usize == before_hand.len() - 1),
                    "illegal legendary spell must not be offered"
                );
            }
        }
    }

    #[test]
    fn issue_166_only_successful_casts_record_generation_bound_facts() {
        let mut e = engine("spell_effect: [GainLife(amount: 1)]");
        let spell = add(&mut e, 0, "snapshot_spell", Zone::Hand);
        assert!(e.apply_scenario_command(0, &command(&e, spell)).is_err());
        assert!(e.state.turn_history.current.spell_casts.is_empty());
        e.state.players[0].mana_pool.black = 1;
        e.apply_scenario_command(0, &command(&e, spell)).unwrap();
        assert_eq!(e.state.turn_history.current.spell_casts.len(), 1);
        let first = e.state.turn_history.current.spell_casts[0].clone();
        assert_eq!(first.origin, Zone::Hand);
        assert_eq!(first.caster, 0);
        assert_eq!(first.types, ["Instant"]);
        assert_eq!(first.mana_value, 1);
        assert_eq!(first.ordinal, 1);
        assert_eq!(
            e.state.stack.last().unwrap().cast_occurrence,
            Some(first.occurrence)
        );
        resolve(&mut e);
        assert_eq!(
            e.state.turn_history.current.spell_casts,
            std::slice::from_ref(&first)
        );
        super::super::resolution::move_object_to_zone(
            &mut e.state,
            e.registry,
            spell,
            Zone::Hand,
            None,
        )
        .unwrap();
        e.state.players[0].mana_pool.black = 1;
        e.apply_scenario_command(0, &command(&e, spell)).unwrap();
        let second = &e.state.turn_history.current.spell_casts[1];
        assert_eq!(second.ordinal, 2);
        assert_ne!(first.occurrence, second.occurrence);
        assert_eq!(first.occurrence.object_id, second.occurrence.object_id);
        e.state.turn_history.finish_turn();
        assert!(e.state.turn_history.current.spell_casts.is_empty());
        assert_eq!(e.state.turn_history.previous.spell_casts.len(), 2);
    }

    fn engine(spell_fields: &str) -> GameEngine {
        engine_with_extra(spell_fields, &[])
    }

    #[test]
    fn issue_238_printed_storm_copies_once_for_each_earlier_cast() {
        let mut e = engine_with_extra(
            "spell_effect: [GainLife(amount: 1)]",
            &[
                r#"(id: "storm_spell", name: "Storm Spell", face_id: "storm_spell",
                mana_cost: "{U}", types: ["Sorcery"], spell_keywords: [Storm],
                spell_effect: [GainLife(amount: 1)])"#,
            ],
        );

        let first = add(&mut e, 0, "snapshot_spell", Zone::Hand);
        e.state.players[0].mana_pool.black = 1;
        e.apply_scenario_command(0, &command(&e, first))
            .expect("first cast");
        resolve(&mut e);

        let storm = add(&mut e, 0, "storm_spell", Zone::Hand);
        e.state.players[0].mana_pool.blue = 1;
        e.state.priority_idx = 0;
        e.apply_scenario_command(0, &command(&e, storm))
            .expect("storm cast");

        assert_eq!(e.state.turn_history.current.spells_cast, 2);
        assert_eq!(
            e.state
                .stack
                .iter()
                .filter(|item| item.is_triggered)
                .count(),
            1,
            "storm must trigger from the spell on the stack"
        );
        resolve(&mut e);
        assert_eq!(
            e.state.players[0].life, 23,
            "the first spell plus one storm copy and the original all resolve"
        );
        assert_eq!(
            e.state.turn_history.current.spells_cast, 2,
            "the storm copy is not cast"
        );
    }

    #[test]
    fn issue_238_each_targeted_storm_copy_gets_an_independent_choice() {
        let mut e = engine_with_extra(
            "spell_effect: [GainLife(amount: 1)]",
            &[
                r#"(id: "targeted_storm", name: "Targeted Storm", face_id: "targeted_storm",
                mana_cost: "{U}", types: ["Instant"], spell_keywords: [Storm],
                spell_effect: [DamageTarget(amount: 1, target: (kind: AnyTarget))])"#,
            ],
        );
        for _ in 0..2 {
            let prior = add(&mut e, 0, "snapshot_spell", Zone::Hand);
            e.state.players[0].mana_pool.black = 1;
            e.apply_scenario_command(0, &command(&e, prior))
                .expect("prior cast");
            resolve(&mut e);
        }

        let storm = add(&mut e, 0, "targeted_storm", Zone::Hand);
        e.state.players[0].mana_pool.blue = 1;
        let mut cast = command(&e, storm);
        let Some(rv1::ruled_command::Cmd::CastSpell(spell)) = cast.cmd.as_mut() else {
            unreachable!()
        };
        spell.targets = vec![rv1::TargetRef {
            object_id: 1,
            ..Default::default()
        }];
        e.apply_scenario_command(0, &cast).expect("storm cast");
        resolve(&mut e);
        assert!(e.state.pending_resolution.is_some(), "first copy choice");
        let original_target = e
            .state
            .stack
            .iter()
            .find(|item| !item.is_copy)
            .expect("original storm spell remains on the stack")
            .targets[0];

        e.apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice::default(),
                )),
            },
        )
        .expect("first copy keeps the original target");
        assert!(
            e.state.pending_resolution.is_some(),
            "second copy must receive its own target choice before priority"
        );
        assert_eq!(e.state.stack.iter().filter(|item| item.is_copy).count(), 1);
        let first_copy = e
            .state
            .stack
            .iter()
            .find(|item| item.is_copy)
            .expect("first copy is pushed before asking for the second");
        assert_eq!(first_copy.targets, vec![original_target]);

        let second_copy_id = e
            .state
            .pending_resolution
            .as_ref()
            .expect("second copy remains pending")
            .presentation
            .source_object_id;
        assert!(e
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![999],
                            ..Default::default()
                        },
                    )),
                },
            )
            .is_err());
        assert_eq!(
            e.state
                .pending_resolution
                .as_ref()
                .expect("invalid choice must preserve the queued copy")
                .presentation
                .source_object_id,
            second_copy_id
        );

        e.apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_player_ids: vec![0],
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("second copy chooses independently");
        assert!(e.state.pending_resolution.is_none());
        assert_eq!(e.state.stack.iter().filter(|item| item.is_copy).count(), 2);
        let copy_targets = e
            .state
            .stack
            .iter()
            .filter(|item| item.is_copy)
            .map(|item| item.targets[0].object_id)
            .collect::<Vec<_>>();
        assert!(copy_targets.contains(&original_target.object_id));
        assert!(copy_targets.contains(&0));
        assert_eq!(e.state.turn_history.current.spells_cast, 3);
    }

    #[test]
    fn issue_238_storm_counts_every_seat_and_keeps_the_original_snapshot_after_countering() {
        let mut e = engine_with_extra(
            "spell_effect: [GainLife(amount: 1)]",
            &[
                r#"(id: "storm_spell", name: "Storm Spell", face_id: "storm_spell",
                mana_cost: "{U}", types: ["Sorcery"], spell_keywords: [Storm],
                spell_effect: [GainLife(amount: 1)])"#,
            ],
        );
        e.state.players.push(PlayerState::new(7, 20));

        for (player_index, player_id) in [(1, 1), (2, 7)] {
            let prior = add(&mut e, player_index, "snapshot_spell", Zone::Hand);
            e.state.players[player_index].mana_pool.black = 1;
            e.state.priority_idx = player_index;
            e.apply_scenario_command(player_id, &command_for(&e, player_index, prior))
                .expect("opponent cast");
            resolve(&mut e);
        }

        let storm = add(&mut e, 0, "storm_spell", Zone::Hand);
        e.state.players[0].mana_pool.blue = 1;
        e.state.priority_idx = 0;
        e.apply_scenario_command(0, &command(&e, storm))
            .expect("storm cast");
        let original_id = e
            .state
            .stack
            .iter()
            .find(|item| !item.is_triggered)
            .expect("original storm spell")
            .id;
        super::super::resolution::counter_stack_object(
            &mut e,
            original_id,
            "test counter",
            &mut Vec::new(),
        )
        .expect("counter original");
        resolve(&mut e);

        assert_eq!(e.state.players[0].life, 22, "two captured copies resolve");
        assert_eq!(e.state.players[1].life, 21);
        assert_eq!(e.state.players[2].life, 21);
        assert_eq!(e.state.turn_history.current.spells_cast, 3);
    }

    #[test]
    fn issue_238_each_printed_or_granted_storm_instance_stages_a_separate_trigger() {
        let mut e = engine_with_extra(
            "spell_effect: [GainLife(amount: 1)]",
            &[
                r#"(id: "storm_spell", name: "Storm Spell", face_id: "storm_spell",
                mana_cost: "{U}", types: ["Sorcery"], spell_keywords: [Storm],
                spell_effect: [GainLife(amount: 1)])"#,
            ],
        );
        for object_id in [900, 901] {
            e.state.static_emblems.push(StaticEmblemInstance {
                object_id,
                controller: 0,
                emblem_id: format!("storm_emblem_{object_id}"),
                display_name: "Storm Emblem".into(),
                effects: vec![StaticEmblemEffect::GrantSpellKeyword {
                    filter: SpellCastFilter {
                        card_type: Some(CardTypeFilter::InstantOrSorcery),
                        ..Default::default()
                    },
                    keyword: SpellKeyword::Storm,
                }],
                triggered_ability: None,
            });
        }
        let storm = add(&mut e, 0, "storm_spell", Zone::Hand);
        e.state.players[0].mana_pool.blue = 1;
        e.apply_scenario_command(0, &command(&e, storm))
            .expect("storm cast");

        let order = e
            .state
            .pending_trigger_order
            .as_ref()
            .expect("three same-controller storm triggers require ordering");
        assert_eq!(order.candidates.len(), 3);
        assert!(order
            .candidates
            .iter()
            .all(|trigger| trigger.ability_text.starts_with("Storm —")));
    }

    #[test]
    fn issue_148_warp_publishes_and_pays_a_distinct_hand_method() {
        let mut e = engine_with_extra(
            "",
            &[
                r#"(id: "warp_test", name: "Warp Test", face_id: "warp_test",
            mana_cost: "{3}{W}", warp_cost: Some("{1}{W}"), types: ["Creature"], power: 3, toughness: 2)"#,
            ],
        );
        let oid = add(&mut e, 0, "warp_test", Zone::Hand);
        let slot = e.state.players[0]
            .hand
            .iter()
            .position(|id| *id == oid)
            .unwrap() as u32;
        let offers = e.initial_response_batch().legal_by_player[&0]
            .hand_actions
            .iter()
            .filter(|a| a.hand_index == slot)
            .map(|a| a.cost.clone())
            .collect::<Vec<_>>();
        assert_eq!(offers, ["{3}{W}", "{1}{W}"]);
        let mut cast = command(&e, oid);
        let Some(rv1::ruled_command::Cmd::CastSpell(ref mut spell)) = cast.cmd else {
            unreachable!()
        };
        spell.cast_method = 5; // CAST_METHOD_WARP, introduced with the coordinated protocol change.
        let before = e.state.command_index;
        assert!(matches!(
            e.apply_scenario_command(0, &cast),
            Err(EngineError::Illegal(_))
        ));
        assert_eq!(e.state.command_index, before);
        assert_eq!(e.state.objects[&oid].zone, Zone::Hand);
        e.state.players[0].mana_pool.white = 1;
        e.state.players[0].mana_pool.colorless = 1;
        e.apply_scenario_command(0, &cast)
            .expect("pay the Warp cost rather than the printed cost");
        assert_eq!(e.state.objects[&oid].zone, Zone::Stack);
        assert_eq!(e.state.players[0].mana_pool.white, 0);
        assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    }

    fn engine_with_extra(spell_fields: &str, extra: &[&str]) -> GameEngine {
        let spell = format!(
            r#"(id: "snapshot_spell", name: "Snapshot Spell", face_id: "snapshot_spell", mana_cost: "{{B}}", types: ["Instant"], {spell_fields})"#
        );
        let mut chunks = vec![
            include_str!("../../../tricerules-cards/data/forest.ron"),
            r#"(id: "snapshot_faerie", name: "Snapshot Faerie", face_id: "snapshot_faerie", types: ["Creature", "Faerie", "Mount"], power: 2, toughness: 2)"#,
            r#"(id: "snapshot_kindred", name: "Snapshot Kindred", face_id: "snapshot_kindred", types: ["Kindred", "Artifact", "Faerie"],)"#,
            r#"(id: "snapshot_siege_snapshot_reverse", name: "Snapshot Siege // Snapshot Reverse", layout: Transform, faces: [
                (name: "Snapshot Siege", face_id: "snapshot_siege", types: ["Battle", "Siege"], defense: 3,
                 cast_conditions: [ActivePlayer(players: Opponents)]),
                (name: "Snapshot Reverse", face_id: "snapshot_reverse", types: ["Sorcery"],
                 cast_conditions: [ActivePlayer(players: Controller)],
                 spell_effect: [GainLife(amount: Conditional(condition: CastSnapshot(index: 0), when_true: 4, otherwise: 1))]),
            ])"#,
            &spell,
        ];
        chunks.extend_from_slice(extra);
        let registry = CardRegistry::from_chunks_and_tokens(&chunks, &[]).unwrap();
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            173010,
            &[0, 1],
            20,
            Some(vec![vec!["forest".into(); 20]; 2]),
            true,
        )
        .unwrap();
        e.registry = Box::leak(Box::new(registry));
        e.state.turn_step = TurnStep::Main1;
        e.state.priority_idx = 0;
        e
    }

    #[test]
    fn issue_166_filters_deduplicate_and_distinguish_event_time_ordinals() {
        let mut e = engine_with_extra(
            "spell_effect: [GainLife(amount: 1)]",
            &[
                r#"(id: "history_watcher", name: "History Watcher", face_id: "history_watcher", types: ["Creature"], power: 1, toughness: 4,
                triggered_abilities: [
                    (ability_id: "triggered_01", presentation: Fallback, trigger: WheneverPlayerCastsSpell(caster: AnyPlayer, filter: (card_type: Some(Noncreature)), ordinal: Some(2)), effect: [GainLife(amount: 1)], ),
                    (ability_id: "triggered_02", presentation: Fallback, trigger: WheneverPlayerCastsSpell(caster: AnyPlayer, filter: (card_type: Some(Noncreature)), ordinal: Some(2), ordinal_scope: MatchingFilter), effect: [GainLife(amount: 1)], ),
                    (ability_id: "triggered_03", presentation: Fallback, trigger: WheneverPlayerCastsSpell(caster: AnyPlayer, filter: (any_of: Some([(card_type: Some(Noncreature)), (required_subtypes: ["Otter"])]))), effect: [GainLife(amount: 1)], ),
                ])"#,
                r#"(id: "history_otter", name: "History Otter", face_id: "history_otter", types: ["Kindred", "Artifact", "Otter"])"#,
            ],
        );
        // The watcher arrives after these casts; matching ordinals still include all prior casts.
        for card in ["snapshot_faerie", "snapshot_spell", "history_otter"] {
            let oid = add(&mut e, 0, card, Zone::Hand);
            e.state.players[0].mana_pool.black = 1;
            e.apply_scenario_command(0, &command(&e, oid)).unwrap();
            resolve(&mut e);
        }
        let watcher = add(&mut e, 1, "history_watcher", Zone::Battlefield);
        e.state.objects.get_mut(&watcher).unwrap().owner = 0;
        let source = e.trigger_source_snapshot(watcher).unwrap();
        let facts = e.state.turn_history.current.spell_casts.clone();
        for (index, expected) in [(0, vec![]), (1, vec![0, 2]), (2, vec![1, 2])] {
            let event = GameEvent::SpellCast {
                fact: facts[index].clone(),
            };
            let collected = e.collect_triggers(&event, std::slice::from_ref(&source));
            assert!(collected.iter().all(|trigger| trigger.controller == 1));
            assert_eq!(
                collected
                    .iter()
                    .map(|t| t.ability_index)
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(e
                .collect_event_triggers(&[event])
                .iter()
                .all(|t| t.trigger_context.affected_player == Some(0)));
        }
        let union = SpellCastFilter {
            any_of: Some(vec![
                SpellCastFilter {
                    card_type: Some(CardTypeFilter::Noncreature),
                    ..Default::default()
                },
                SpellCastFilter {
                    required_subtypes: vec!["Otter".into()],
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };
        assert_eq!(
            super::super::history::spell_cast_count(
                &e.state,
                ConditionPlayerSet::Relative(RelativePlayerSet::All),
                &union,
                0,
                None,
                false
            ),
            2
        );
        assert_eq!(
            super::super::history::spell_cast_count(
                &e.state,
                ConditionPlayerSet::AffectedPlayer,
                &union,
                0,
                None,
                false
            ),
            0,
            "missing affected-player context cannot fall back to the controller"
        );
    }

    #[test]
    fn issue_191_cast_filter_matches_one_committed_creature_target() {
        let mut e = engine_with_extra(
            "spell_effect: [PumpTarget(power: 1, toughness: 1, subject: Chosen((kind: Creature)))]",
            &[
                r#"(id: "repartee_watcher", name: "Repartee Watcher", face_id: "repartee_watcher", types: ["Creature"], power: 1, toughness: 1,
                triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
                    trigger: WheneverPlayerCastsSpell(caster: Controller, filter: (card_type: Some(InstantOrSorcery), targeted_permanent_type: Some(Creature))),
                    effect: [GainLife(amount: 1)])])"#,
            ],
        );
        add(&mut e, 0, "repartee_watcher", Zone::Battlefield);
        let target = add(&mut e, 1, "snapshot_faerie", Zone::Battlefield);
        let spell = add(&mut e, 0, "snapshot_spell", Zone::Hand);
        e.state.players[0].mana_pool.black = 1;
        let mut cast = command(&e, spell);
        let Some(rv1::ruled_command::Cmd::CastSpell(ref mut command)) = cast.cmd else {
            unreachable!()
        };
        command.targets = vec![rv1::TargetRef {
            object_id: target,
            kind: rv1::TargetRefKind::Permanent as i32,
            ..Default::default()
        }];
        e.apply_scenario_command(0, &cast)
            .expect("cast at a creature");
        assert_eq!(
            e.state.stack.len(),
            2,
            "Repartee triggers once above the spell"
        );
        assert!(e.state.stack.last().unwrap().is_triggered);
    }

    fn issue_191_watcher() -> &'static str {
        r#"(id: "repartee_watcher", name: "Repartee Watcher", face_id: "repartee_watcher", types: ["Creature"], power: 1, toughness: 1,
            triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
                trigger: WheneverPlayerCastsSpell(caster: Controller, filter: (card_type: Some(InstantOrSorcery), targeted_permanent_type: Some(Creature))),
                effect: [GainLife(amount: 1)])])"#
    }

    #[test]
    fn issue_191_repeated_and_mixed_targets_match_once_and_snapshot_immutably() {
        for second_target_is_player in [false, true] {
            let mut e = engine_with_extra(
                r#"spell_effect: [
                    PumpTarget(power: 1, toughness: 1, subject: Chosen((kind: Creature))),
                    DamageTarget(amount: 1, target: (kind: AnyTarget)),
                ],
                targeting: Some((groups: [
                    (min: 1, max: 1, prompt: "Choose target creature", effect_indices: [0]),
                    (min: 1, max: 1, prompt: "Choose any target", effect_indices: [1]),
                ]))"#,
                &[issue_191_watcher()],
            );
            add(&mut e, 0, "repartee_watcher", Zone::Battlefield);
            let target = add(&mut e, 1, "snapshot_faerie", Zone::Battlefield);
            let spell = add(&mut e, 0, "snapshot_spell", Zone::Hand);
            e.state.players[0].mana_pool.black = 1;
            let mut cast = command(&e, spell);
            let Some(rv1::ruled_command::Cmd::CastSpell(ref mut cast_spell_command)) = cast.cmd
            else {
                unreachable!()
            };
            cast_spell_command.targets = vec![
                rv1::TargetRef {
                    object_id: target,
                    group_index: 0,
                    kind: rv1::TargetRefKind::Permanent as i32,
                    ..Default::default()
                },
                rv1::TargetRef {
                    object_id: if second_target_is_player { 1 } else { target },
                    group_index: 1,
                    kind: if second_target_is_player {
                        rv1::TargetRefKind::Player as i32
                    } else {
                        rv1::TargetRefKind::Permanent as i32
                    },
                    ..Default::default()
                },
            ];
            e.apply_scenario_command(0, &cast)
                .expect("cast with two target roles");
            assert_eq!(e.state.stack.len(), 2, "one trigger per cast event");
            let fact = e
                .state
                .turn_history
                .current
                .spell_casts
                .last()
                .unwrap()
                .clone();
            assert_eq!(
                fact.targeted_permanent_types,
                [PermanentTypeFilter::Creature]
            );

            e.state.players[1].battlefield.retain(|oid| *oid != target);
            e.state.players[1].hand.push(target);
            e.state.objects.get_mut(&target).unwrap().zone = Zone::Hand;
            *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
            let filter = SpellCastFilter {
                card_type: Some(CardTypeFilter::InstantOrSorcery),
                targeted_permanent_type: Some(PermanentTypeFilter::Creature),
                ..Default::default()
            };
            assert!(
                super::super::history::spell_cast_matches(&filter, &fact),
                "later target changes do not rewrite the committed cast fact"
            );

            let facts_before_copy = e.state.turn_history.current.spell_casts.len();
            let original = e
                .state
                .stack
                .iter()
                .find(|item| !item.is_triggered)
                .unwrap();
            let mut copy = original.clone();
            copy.id = e.state.next_object_id;
            e.state.next_object_id += 1;
            copy.is_copy = true;
            copy.cast_occurrence = None;
            e.state.stack.push(copy);
            assert_eq!(
                e.state.turn_history.current.spell_casts.len(),
                facts_before_copy,
                "a direct stack copy is not cast"
            );
        }
    }

    #[test]
    fn issue_191_player_targets_and_targeted_permanent_spells_do_not_match() {
        let mut player_target = engine_with_extra(
            "spell_effect: [DamageTarget(amount: 1, target: (kind: AnyTarget))]",
            &[issue_191_watcher()],
        );
        add(&mut player_target, 0, "repartee_watcher", Zone::Battlefield);
        let spell = add(&mut player_target, 0, "snapshot_spell", Zone::Hand);
        player_target.state.players[0].mana_pool.black = 1;
        let mut cast = command(&player_target, spell);
        let Some(rv1::ruled_command::Cmd::CastSpell(ref mut cast_spell_command)) = cast.cmd else {
            unreachable!()
        };
        cast_spell_command.targets = vec![rv1::TargetRef {
            object_id: 1,
            kind: rv1::TargetRefKind::Player as i32,
            ..Default::default()
        }];
        player_target.apply_scenario_command(0, &cast).unwrap();
        assert_eq!(player_target.state.stack.len(), 1);
        assert!(player_target.state.turn_history.current.spell_casts[0]
            .targeted_permanent_types
            .is_empty());

        let aura = r#"(id: "snapshot_aura", name: "Snapshot Aura", face_id: "snapshot_aura", mana_cost: "{B}", types: ["Enchantment", "Aura"], spell_effect: [AuraAttach(target: (kind: Creature))])"#;
        let mut permanent_spell = engine_with_extra(
            "spell_effect: [GainLife(amount: 1)]",
            &[issue_191_watcher(), aura],
        );
        add(
            &mut permanent_spell,
            0,
            "repartee_watcher",
            Zone::Battlefield,
        );
        let target = add(
            &mut permanent_spell,
            1,
            "snapshot_faerie",
            Zone::Battlefield,
        );
        let aura = add(&mut permanent_spell, 0, "snapshot_aura", Zone::Hand);
        permanent_spell.state.players[0].mana_pool.black = 1;
        let mut cast = command(&permanent_spell, aura);
        let Some(rv1::ruled_command::Cmd::CastSpell(ref mut cast_spell_command)) = cast.cmd else {
            unreachable!()
        };
        cast_spell_command.targets = vec![rv1::TargetRef {
            object_id: target,
            kind: rv1::TargetRefKind::Permanent as i32,
            ..Default::default()
        }];
        permanent_spell.apply_scenario_command(0, &cast).unwrap();
        assert_eq!(permanent_spell.state.stack.len(), 1);
        assert_eq!(
            permanent_spell.state.turn_history.current.spell_casts[0].targeted_permanent_types,
            [PermanentTypeFilter::Creature]
        );
    }

    #[test]
    fn issue_191_target_sacrificed_during_cast_is_absent_at_cast_commitment() {
        let mut e = engine_with_extra(
            r#"additional_costs: [SacrificePermanent(filter: (kind: Creature, controller: You))],
                spell_effect: [PumpTarget(power: 1, toughness: 1, subject: Chosen((kind: Creature)))]"#,
            &[issue_191_watcher()],
        );
        add(&mut e, 0, "repartee_watcher", Zone::Battlefield);
        let target = add(&mut e, 0, "snapshot_faerie", Zone::Battlefield);
        let spell = add(&mut e, 0, "snapshot_spell", Zone::Hand);
        e.state.players[0].mana_pool.black = 1;
        let mut cast = command(&e, spell);
        let Some(rv1::ruled_command::Cmd::CastSpell(ref mut cast_spell_command)) = cast.cmd else {
            unreachable!()
        };
        cast_spell_command.targets = vec![rv1::TargetRef {
            object_id: target,
            kind: rv1::TargetRefKind::Permanent as i32,
            ..Default::default()
        }];
        cast_spell_command.cost_selections = vec![rv1::CostSelection {
            cost_index: 0,
            selection: Some(rv1::cost_selection::Selection::PermanentId(target)),
        }];
        e.apply_scenario_command(0, &cast)
            .expect("sacrifice the chosen target while casting");
        assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
        assert_eq!(e.state.stack.len(), 1, "Repartee does not trigger");
        assert!(e.state.turn_history.current.spell_casts[0]
            .targeted_permanent_types
            .is_empty());
    }

    #[test]
    fn issue_464_damage_source_predicate_filters_additional_sacrifice_candidates() {
        let mut e = engine_with_extra(
            r#"additional_costs: [SacrificePermanent(filter: (kind: Creature, controller: You, dealt_damage_this_turn: Some(true)))],
                spell_effect: [GainLife(amount: 1)]"#,
            &[],
        );
        let dealt_damage = add(&mut e, 0, "snapshot_faerie", Zone::Battlefield);
        let no_damage = add(&mut e, 0, "snapshot_faerie", Zone::Battlefield);
        let opponent_damage = add(&mut e, 1, "snapshot_faerie", Zone::Battlefield);
        let spell = add(&mut e, 0, "snapshot_spell", Zone::Hand);
        let generation = e
            .state
            .zone_change_generation
            .get(&dealt_damage)
            .copied()
            .unwrap_or(0);
        let opponent_generation = e
            .state
            .zone_change_generation
            .get(&opponent_damage)
            .copied()
            .unwrap_or(0);
        let mut dealt_to_player = damage::DamageEvent::noncombat(
            dealt_damage,
            0,
            "Snapshot Faerie",
            damage::DamageRecipient::Player(1),
            1,
        );
        dealt_to_player.source.zone_change_generation = Some(generation);
        let mut only_received = damage::DamageEvent::noncombat(
            opponent_damage,
            1,
            "Snapshot Faerie",
            damage::DamageRecipient::Permanent(no_damage),
            1,
        );
        only_received.source.zone_change_generation = Some(opponent_generation);
        e.fire_triggers(
            &[
                GameEvent::DamageDealt {
                    event: dealt_to_player,
                },
                GameEvent::DamageDealt {
                    event: only_received,
                },
            ],
            &mut Vec::new(),
        );
        e.state.players[1].has_lost = true;
        assert!(e.state.turn_history.current.damaged_objects.contains(&(
            no_damage,
            e.state
                .zone_change_generation
                .get(&no_damage)
                .copied()
                .unwrap_or(0),
        )));
        assert!(e
            .state
            .turn_history
            .current
            .dealt_damage_objects
            .contains(&(dealt_damage, generation)));
        e.state.players[0].mana_pool.black = 1;

        for rejected_candidate in [no_damage, opponent_damage] {
            let mut rejected = command(&e, spell);
            let Some(rv1::ruled_command::Cmd::CastSpell(ref mut cast)) = rejected.cmd else {
                unreachable!()
            };
            cast.cost_selections = vec![rv1::CostSelection {
                cost_index: 0,
                selection: Some(rv1::cost_selection::Selection::PermanentId(
                    rejected_candidate,
                )),
            }];
            assert!(matches!(
                e.apply_scenario_command(0, &rejected),
                Err(EngineError::Illegal(_))
            ));
            assert_eq!(e.state.objects[&spell].zone, Zone::Hand);
        }

        let mut accepted = command(&e, spell);
        let Some(rv1::ruled_command::Cmd::CastSpell(ref mut cast)) = accepted.cmd else {
            unreachable!()
        };
        cast.cost_selections = vec![rv1::CostSelection {
            cost_index: 0,
            selection: Some(rv1::cost_selection::Selection::PermanentId(dealt_damage)),
        }];
        e.apply_scenario_command(0, &accepted)
            .expect("the generation recorded as a damage source is a legal sacrifice");
        assert_eq!(e.state.objects[&dealt_damage].zone, Zone::Graveyard);
        assert_eq!(e.state.objects[&no_damage].zone, Zone::Battlefield);
        assert_eq!(e.state.objects[&opponent_damage].zone, Zone::Battlefield);
    }

    #[test]
    fn issue_166_history_preserves_cast_faces_origins_and_subtypes() {
        let mut e = engine_with_extra(
            r#"flashback_cost: Some("{B}"),
                cast_conditions: [SpellsCastThisTurn(players: Controller, filter: (origin: Some(Graveyard)), min: Some(1))],
                spell_effect: [GainLife(amount: 1)]"#,
            &[
                include_str!(
                    "../../../tricerules-cards/data/multiface/bonecrusher_giant_stomp.ron"
                ),
                r#"(id: "history_shifter", name: "History Shifter", face_id: "history_shifter", types: ["Creature", "Shapeshifter"], power: 1, toughness: 1, characteristic_defining_abilities: [(ability_id: "characteristic_01", presentation: Fallback, definition: Changeling)])"#,
                r#"(id: "history_static", name: "History Static", face_id: "history_static", types: ["Creature"], power: 1, toughness: 1,
                    static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: ConditionalSelfModifier(condition: SpellsCastThisTurn(players: Controller, filter: (origin: Some(Graveyard)), min: Some(1)), delta_power: 2))])"#,
            ],
        );
        let permanent = add(&mut e, 0, "history_static", Zone::Battlefield);
        e.fire_triggers(
            &[GameEvent::EntersBattlefield {
                object_id: permanent,
                chosen_x: 0,
                entry_cause: None,
            }],
            &mut Vec::new(),
        );
        let spell = add(&mut e, 0, "snapshot_spell", Zone::Hand);
        e.state.players[0].mana_pool.black = 2;
        e.apply_scenario_command(0, &command(&e, spell)).unwrap();
        assert_eq!(
            e.state.stack.last().unwrap().cast_condition_results,
            [false]
        );
        resolve(&mut e);
        assert_eq!(e.effective_power(permanent), Some(1));
        let flashback = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                cast_method: rv1::CastMethod::Flashback as i32,
                source: Some(rv1::CastSource {
                    expected_zone_change_generation: Some(
                        e.state
                            .zone_change_generation
                            .get(&spell)
                            .copied()
                            .unwrap_or(0),
                    ),
                    location: Some(rv1::cast_source::Location::GraveyardObjectId(spell)),
                }),
                ..Default::default()
            })),
        };
        e.apply_scenario_command(0, &flashback).unwrap();
        assert_eq!(e.state.stack.last().unwrap().cast_condition_results, [true]);
        resolve(&mut e);
        assert_eq!(e.effective_power(permanent), Some(3));
        let before = e.state.turn_history.clone();
        assert!(e.apply_scenario_command(0, &flashback).is_err());
        assert_eq!(
            e.state.turn_history, before,
            "stale source contributes nothing"
        );

        let adventure = add(&mut e, 0, "bonecrusher_giant_stomp", Zone::Hand);
        let mut cast = command(&e, adventure);
        let Some(rv1::ruled_command::Cmd::CastSpell(cs)) = cast.cmd.as_mut() else {
            unreachable!()
        };
        cs.face_index = 1;
        cs.targets = vec![rv1::TargetRef {
            object_id: 1,
            kind: rv1::TargetRefKind::Player as i32,
            ..Default::default()
        }];
        e.state.players[0].mana_pool.red = 5;
        e.apply_scenario_command(0, &cast).unwrap();
        resolve(&mut e);
        let permission_id = e
            .state
            .active_exile_play_permissions
            .iter()
            .find(|permission| permission.object_id == adventure)
            .expect("Adventure permission")
            .group_id;
        let creature_cast = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                cast_method: rv1::CastMethod::Normal as i32,
                source: Some(rv1::CastSource {
                    expected_zone_change_generation: Some(
                        e.state
                            .zone_change_generation
                            .get(&adventure)
                            .copied()
                            .unwrap_or(0),
                    ),
                    location: Some(rv1::cast_source::Location::ExileObjectId(adventure)),
                }),
                casting_permission_id: Some(permission_id),
                ..Default::default()
            })),
        };
        e.apply_scenario_command(0, &creature_cast).unwrap();
        resolve(&mut e);
        let shifter = add(&mut e, 0, "history_shifter", Zone::Hand);
        e.apply_scenario_command(0, &command(&e, shifter)).unwrap();
        resolve(&mut e);
        let facts = &e.state.turn_history.current.spell_casts;
        assert_eq!(
            facts.iter().map(|f| f.origin).collect::<Vec<_>>(),
            [
                Zone::Hand,
                Zone::Graveyard,
                Zone::Hand,
                Zone::Exile,
                Zone::Hand
            ]
        );
        assert_eq!(facts[2].face_index, 1);
        assert_eq!(facts[3].face_index, 0);
        assert_ne!(facts[2].occurrence, facts[3].occurrence);
        let count = |filter: SpellCastFilter| {
            super::super::history::spell_cast_count(
                &e.state,
                ConditionPlayerSet::Relative(RelativePlayerSet::Controller),
                &filter,
                0,
                None,
                false,
            )
        };
        assert_eq!(
            count(SpellCastFilter {
                card_type: Some(CardTypeFilter::Noncreature),
                ..Default::default()
            }),
            3
        );
        assert_eq!(
            count(SpellCastFilter {
                required_subtypes: vec!["Adventure".into()],
                ..Default::default()
            }),
            1
        );
        assert_eq!(
            count(SpellCastFilter {
                required_subtypes: vec!["Otter".into()],
                ..Default::default()
            }),
            1
        );
        assert_eq!(
            count(SpellCastFilter {
                min_mana_value: Some(2),
                max_mana_value: Some(2),
                origin: Some(SpellCastOrigin::Hand),
                ..Default::default()
            }),
            1
        );
        assert_eq!(
            count(SpellCastFilter {
                origin: Some(SpellCastOrigin::Graveyard),
                ..Default::default()
            }),
            1
        );
        assert_eq!(
            count(SpellCastFilter {
                origin: Some(SpellCastOrigin::Exile),
                ..Default::default()
            }),
            1
        );
        // History rollover is independent of seat changes, including a subsequent turn
        // belonging to the same player; adding extra-turn scheduling is outside this issue.
        let active = e.state.active_player_id();
        e.state.turn_history.finish_turn();
        assert_eq!(e.state.active_player_id(), active);
        assert_eq!(e.effective_power(permanent), Some(1));
    }

    #[test]
    fn issue_166_cast_copy_commit_is_distinct_from_direct_copy_placement() {
        let mut e = engine("spell_effect: [GainLife(amount: 1)]");
        e.state.players.push(PlayerState::new(7, 20));
        let spell = add(&mut e, 0, "snapshot_spell", Zone::Hand);
        e.state.players[0].mana_pool.black = 1;
        e.apply_scenario_command(0, &command(&e, spell)).unwrap();
        let mut copy = e.state.stack.last().unwrap().clone();
        copy.id = e.state.next_object_id;
        e.state.next_object_id += 1;
        copy.controller = 7;
        copy.is_copy = true;
        copy.cast_occurrence = None;
        let copy_id = copy.id;
        e.state.stack.push(copy);
        assert_eq!(e.state.turn_history.current.spell_casts.len(), 1);
        // Exercise the commit seam, not a new cast-copy permission or gameplay flow.
        let fact = e.record_spell_cast(7, copy_id, Zone::Exile, 1, 0);
        assert_eq!(fact.occurrence.zone_change_generation, None);
        assert_eq!(
            e.state.stack.last().unwrap().cast_occurrence,
            Some(fact.occurrence)
        );
        assert_eq!(e.state.turn_history.current.player(7).spells_cast, 1);
        assert_eq!(e.state.turn_history.current.player(1).spells_cast, 0);
        let quantity = CountExpression::SpellsCastThisTurn {
            players: ConditionPlayerSet::Relative(RelativePlayerSet::Opponents),
            filter: Default::default(),
            exclude_source: false,
        };
        assert_eq!(
            e.resolve_amount(
                &Amount::Count(quantity),
                AmountContext::for_stack_item(&e.state.stack[0], 0)
            ),
            1
        );
    }

    fn add(e: &mut GameEngine, player: usize, card: &str, zone: Zone) -> ObjectId {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        let owner = e.state.players[player].id;
        let face = e.registry.get(card).unwrap().primary_face();
        let mut object = new_object_from_card(id, owner, card, zone, face);
        object.summoning_sick = false;
        e.state.objects.insert(id, object);
        match zone {
            Zone::Hand => e.state.players[player].hand.push(id),
            Zone::Battlefield => e.state.players[player].battlefield.push(id),
            Zone::Graveyard => e.state.players[player].graveyard.push(id),
            _ => panic!("unsupported fixture zone"),
        }
        id
    }

    fn command(e: &GameEngine, spell: ObjectId) -> RuledCommand {
        command_for(e, 0, spell)
    }

    fn command_for(e: &GameEngine, player_index: usize, spell: ObjectId) -> RuledCommand {
        RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                cast_method: rv1::CastMethod::Normal as i32,
                source: Some(rv1::CastSource {
                    expected_zone_change_generation: None,
                    location: Some(rv1::cast_source::Location::HandIndex(
                        e.state.players[player_index]
                            .hand
                            .iter()
                            .position(|id| *id == spell)
                            .unwrap() as u32,
                    )),
                }),
                ..Default::default()
            })),
        }
    }

    fn resolve(e: &mut GameEngine) {
        while !e.state.stack.is_empty() && e.state.pending_resolution.is_none() {
            let player = e.state.priority_player_id();
            e.apply_scenario_command(
                player,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
        }
    }

    const FAERIE: &str = r#"BattlefieldAggregate(filter: (controllers: Controller, required_subtypes: ["Faerie"]), aggregate: Count, min: Some(1))"#;

    #[test]
    fn issue_173_payment_and_cast_history_are_committed_before_capture() {
        use prost::Message;
        fn run(reject_first: bool) -> (Vec<RuledEventBatch>, Vec<bool>, i32) {
            let mut e = engine(&format!(
                r#"
                additional_costs: [SacrificePermanent(filter: (kind: Creature, controller: You))],
                cast_conditions: [{FAERIE}, CreatureDeathsThisTurn(min: Some(1)), SpellsCastThisTurn(players: Controller, min: Some(1)), PermanentCardsEnteredGraveyardThisTurn(players: Controller, min: Some(1)), PermanentsSacrificedThisTurn(players: Controller, permanent_type: Some(Creature), min: Some(1))],
                spell_effect: [GainLife(amount: Conditional(condition: CastSnapshot(index: 0), when_true: 4, otherwise: 2))],
            "#
            ));
            let faerie = add(&mut e, 0, "snapshot_faerie", Zone::Battlefield);
            let spell = add(&mut e, 0, "snapshot_spell", Zone::Hand);
            let mut cast = command(&e, spell);
            let Some(rv1::ruled_command::Cmd::CastSpell(cs)) = cast.cmd.as_mut() else {
                unreachable!()
            };
            cs.cost_selections = vec![rv1::CostSelection {
                cost_index: 0,
                selection: Some(rv1::cost_selection::Selection::PermanentId(faerie)),
            }];
            let cast = RuledCommand::decode(cast.encode_to_vec().as_slice()).unwrap();
            if reject_first {
                let index = e.state.command_index;
                assert!(e.apply_scenario_command(0, &cast).is_err());
                assert_eq!(e.state.command_index, index);
                assert_eq!(e.state.objects[&faerie].zone, Zone::Battlefield);
                assert_eq!(e.state.objects[&spell].zone, Zone::Hand);
                assert!(e.state.stack.is_empty());
                assert_eq!(e.state.turn_history.current.creatures_died, 0);
                assert!(e
                    .state
                    .turn_history
                    .current
                    .permanent_cards_entered_graveyard
                    .is_empty());
                assert!(e
                    .state
                    .turn_history
                    .current
                    .permanents_sacrificed
                    .is_empty());
            }
            e.state.players[0].mana_pool.black = 1;
            let mut batches = vec![e.apply_scenario_command(0, &cast).unwrap()];
            let snapshots = e.state.stack.last().unwrap().cast_condition_results.clone();
            assert_eq!(snapshots, vec![false, true, true, true, true]);
            assert_eq!(e.state.objects[&faerie].zone, Zone::Graveyard);
            while !e.state.stack.is_empty() {
                let player = e.state.priority_player_id();
                batches.push(
                    e.apply_scenario_command(
                        player,
                        &RuledCommand {
                            cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                        },
                    )
                    .unwrap(),
                );
            }
            (batches, snapshots, e.state.players[0].life)
        }
        let accepted = run(false);
        assert_eq!(accepted.2, 22);
        assert_eq!(
            accepted,
            run(true),
            "rejected attempts do not alter accepted-command replay"
        );
    }

    #[test]
    fn issue_173_parked_program_preserves_cast_snapshot_after_live_change() {
        let mut e = engine(&format!(
            r#"
            cast_conditions: [{FAERIE}],
            spell_program: Some((body: (steps: [
                (id: "scry", bind: [], input: None, kind: Perform(Scry(count: 1))),
                (id: "cast_snapshot_gate", bind: [], input: None, kind: If(
                    condition: CastSnapshot(index: 0),
                    yes: (steps: [(id: "snapshot_bonus", bind: [], input: None, kind: Perform(GainLife(amount: 4)))], yields: []),
                    no: (steps: [(id: "fallback_bonus", bind: [], input: None, kind: Perform(GainLife(amount: 1)))], yields: []),
                )),
                (id: "live_condition", bind: [], input: None, kind: If(
                    condition: {FAERIE},
                    yes: (steps: [(id: "live_bonus", bind: [], input: None, kind: Perform(GainLife(amount: 8)))], yields: []),
                    no: (steps: [(id: "live_fallback", bind: [], input: None, kind: Perform(GainLife(amount: 2)))], yields: []),
                )),
            ], yields: []))),
        "#
        ));
        let faerie = add(&mut e, 0, "snapshot_faerie", Zone::Battlefield);
        let spell = add(&mut e, 0, "snapshot_spell", Zone::Hand);
        let cast = command(&e, spell);
        e.state.players[0].mana_pool.black = 1;
        e.apply_scenario_command(0, &cast).unwrap();
        resolve(&mut e);
        assert!(e.state.pending_resolution.is_some());
        super::super::resolution::move_object_to_zone(
            &mut e.state,
            e.registry,
            faerie,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        let answer = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![],
                    ..Default::default()
                },
            )),
        };
        assert!(e.apply_scenario_command(1, &answer).is_err());
        e.apply_scenario_command(0, &answer).unwrap();
        resolve(&mut e);
        assert!(e.state.pending_resolution.is_none());
        assert_eq!(
            e.state.players[0].life, 26,
            "frozen bonus + live fallback, exactly once"
        );
        assert!(
            e.apply_scenario_command(0, &answer).is_err(),
            "completed continuation cannot be replayed"
        );
    }

    #[test]
    fn captured_modal_identity_survives_definition_reordering_and_presentation_changes() {
        let mut e = engine(
            r#"modal_spell: (min_modes: 1, max_modes: 1, modes: [
                (mode_id: "gain", presentation: OracleLines([2]), effects: [GainLife(amount: 1)]),
                (mode_id: "draw", presentation: Fallback, effects: [Draw(count: 1)]),
            ])"#,
        );
        let spell = add(&mut e, 0, "snapshot_spell", Zone::Hand);
        e.state.players[0].mana_pool.black = 1;
        let mut cast = command(&e, spell);
        let Some(rv1::ruled_command::Cmd::CastSpell(ref mut command)) = cast.cmd else {
            unreachable!()
        };
        command.selected_modes = vec![rv1::SelectedSpellMode {
            mode_index: 0,
            targets: vec![],
        }];
        e.apply_scenario_command(0, &cast).unwrap();
        assert_eq!(
            e.state.stack.last().unwrap().chosen_modes[0]
                .mode_id
                .as_str(),
            "gain"
        );

        let reordered = r#"(
            id: "snapshot_spell", name: "Snapshot Spell", face_id: "snapshot_spell",
            mana_cost: "{B}", types: ["Instant"],
            modal_spell: (min_modes: 1, max_modes: 1, modes: [
                (mode_id: "draw", presentation: OracleLines([1]), effects: [Draw(count: 1)]),
                (mode_id: "gain", presentation: Fallback, effects: [GainLife(amount: 1)]),
            ]),
        )"#;
        e.registry = Box::leak(Box::new(
            CardRegistry::from_chunks_and_tokens(
                &[
                    include_str!("../../../tricerules-cards/data/forest.ron"),
                    reordered,
                ],
                &[],
            )
            .unwrap(),
        ));

        resolve(&mut e);
        assert_eq!(e.state.players[0].life, 21);
    }

    #[test]
    fn issue_173_snapshots_use_controllers_and_noncreature_subtypes() {
        let mut e = engine(&format!(
            r#"cast_conditions: [{FAERIE}], spell_effect: [GainLife(amount: 1)]"#
        ));
        e.state.players.push(PlayerState::new(99, 20));
        e.state.next_object_id = 100;
        let kindred = add(&mut e, 2, "snapshot_kindred", Zone::Battlefield);
        e.state.players[2].battlefield.clear();
        e.state.players[0].battlefield.push(kindred);
        e.state.objects.get_mut(&kindred).unwrap().base_controller = 0;
        e.state.objects.get_mut(&kindred).unwrap().controller = 0;
        let spell = add(&mut e, 0, "snapshot_spell", Zone::Hand);
        e.state.players[0].mana_pool.black = 1;
        e.apply_scenario_command(0, &command(&e, spell)).unwrap();
        assert_eq!(
            e.state.stack.last().unwrap().cast_condition_results,
            vec![true]
        );
        assert_eq!(e.state.objects[&kindred].owner, 99);
    }

    #[test]
    fn issue_173_snapshot_can_read_the_selected_target_generation() {
        let mut e = engine(
            r#"
            cast_conditions: [ObjectWasDealtDamageThisTurn(object: ChosenTarget(group_index: 0, target_index: 0))],
            spell_effect: [DamageTarget(target: (kind: Creature), amount: Conditional(condition: CastSnapshot(index: 0), when_true: 1, otherwise: 0))],
        "#,
        );
        let target = add(&mut e, 1, "snapshot_faerie", Zone::Battlefield);
        e.state
            .turn_history
            .current
            .damaged_objects
            .push((target, 0));
        let spell = add(&mut e, 0, "snapshot_spell", Zone::Hand);
        let mut cast = command(&e, spell);
        let Some(rv1::ruled_command::Cmd::CastSpell(cs)) = cast.cmd.as_mut() else {
            unreachable!()
        };
        cs.targets = vec![rv1::TargetRef {
            object_id: target,
            ..Default::default()
        }];
        e.state.players[0].mana_pool.black = 1;
        e.apply_scenario_command(0, &cast).unwrap();
        assert_eq!(
            e.state.stack.last().unwrap().cast_condition_results,
            vec![true]
        );
    }

    #[test]
    fn issue_173_siege_offer_snapshots_the_cast_face() {
        let mut e = engine("spell_effect: [GainLife(amount: 1)]");
        let siege = add(
            &mut e,
            0,
            "snapshot_siege_snapshot_reverse",
            Zone::Battlefield,
        );
        e.state.objects.get_mut(&siege).unwrap().counters.clear();
        e.stage_siege_defeat_trigger(siege);
        e.flush_staged_triggers(&mut vec![]);
        resolve(&mut e);
        let generation = e.state.zone_change_generation[&siege];
        let permission = e
            .special_cast_permission(0)
            .expect("Siege resolution cast offer");
        let command = rv1::CastSpell {
            cast_method: rv1::CastMethod::SiegeDefeat as i32,
            casting_permission_id: Some(permission.group_id),
            source: Some(rv1::CastSource {
                expected_zone_change_generation: Some(generation),
                location: Some(rv1::cast_source::Location::ExileObjectId(siege)),
            }),
            face_index: 1,
            ..Default::default()
        };
        e.submit_resolution_choice(
            0,
            &rv1::SubmitResolutionChoice {
                decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                cast_spell: Some(command),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            e.state.stack.last().unwrap().cast_condition_results,
            vec![true]
        );
        let fact = &e.state.turn_history.current.spell_casts[0];
        assert_eq!(fact.origin, Zone::Exile);
        assert_eq!(fact.face_index, 1);
        assert_eq!(fact.types, ["Sorcery"]);
        assert_eq!(fact.occurrence.zone_change_generation, Some(generation + 1));
        resolve(&mut e);
        assert_eq!(e.state.players[0].life, 24);
    }

    const SOURCE_LOYALTY_TIMING_PROBE: &str = r#"(
        id: "source_loyalty_timing_probe", name: "Source Loyalty Timing Probe",
        face_id: "source_loyalty_timing_probe", types: ["Planeswalker"], loyalty: Some(3),
        static_abilities: [(ability_id: "static_01", presentation: Fallback,
            definition: SourceLoyaltyAbilitiesAtInstantTiming)],
        activated_abilities: [
            (ability_id: "plus_01", presentation: Fallback,
                costs: [Loyalty(1)], effect: [GainLife(amount: 1)]),
            (ability_id: "strict_01", presentation: Fallback,
                costs: [Loyalty(1)], effect: [GainLife(amount: 1)], timing: SorcerySpeed)
        ]
    )"#;

    const ORDINARY_LOYALTY_TIMING_PROBE: &str = r#"(
        id: "ordinary_loyalty_timing_probe", name: "Ordinary Loyalty Timing Probe",
        face_id: "ordinary_loyalty_timing_probe", types: ["Planeswalker"], loyalty: Some(3),
        activated_abilities: [(ability_id: "plus_01", presentation: Fallback,
            costs: [Loyalty(1)], effect: [GainLife(amount: 1)])]
    )"#;

    const EQUIP_TIMING_PROBE: &str = r#"(
        id: "equip_timing_probe", name: "Equip Timing Probe",
        face_id: "equip_timing_probe", types: ["Artifact", "Equipment"],
        activated_abilities: [(ability_id: "equip_01", presentation: Fallback,
            costs: [Mana("{2}")],
            effect: [Equip(target: (kind: Creature, controller: You))])]
    )"#;

    fn source_loyalty_timing_fixture() -> (GameEngine, ObjectId, ObjectId, ObjectId) {
        let mut e = engine_with_extra(
            "",
            &[
                SOURCE_LOYALTY_TIMING_PROBE,
                ORDINARY_LOYALTY_TIMING_PROBE,
                EQUIP_TIMING_PROBE,
            ],
        );
        let source = add(&mut e, 1, "source_loyalty_timing_probe", Zone::Battlefield);
        let ordinary = add(
            &mut e,
            1,
            "ordinary_loyalty_timing_probe",
            Zone::Battlefield,
        );
        let equip = add(&mut e, 1, "equip_timing_probe", Zone::Battlefield);
        e.state
            .objects
            .get_mut(&source)
            .expect("source was inserted into the fixture")
            .set_counter(CounterKind::Loyalty, 3);
        e.state
            .objects
            .get_mut(&ordinary)
            .expect("comparison walker was inserted into the fixture")
            .set_counter(CounterKind::Loyalty, 3);
        e.reconcile_activated_ability_slots();
        e.state.turn_step = TurnStep::Upkeep;
        e.state.players[0].mana_pool.black = 1;
        let stack_spell = add(&mut e, 0, "snapshot_spell", Zone::Hand);
        let active_player = e.state.players[0].id;
        e.apply_scenario_command(active_player, &command(&e, stack_spell))
            .expect("the active player puts an instant on the stack during upkeep");
        e.apply_scenario_command(
            active_player,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
            },
        )
        .expect("priority passes to the source controller while the instant remains on stack");
        (e, source, ordinary, equip)
    }

    fn active_ability_by_id(
        engine: &GameEngine,
        source: ObjectId,
        ability_id: &str,
    ) -> (u32, tricerules_card_model::ActivatedAbilityDef) {
        let ability = engine
            .effective_activated_abilities(source)
            .into_iter()
            .find(|ability| ability.definition.ability_id.as_str() == ability_id)
            .unwrap_or_else(|| panic!("missing active ability {ability_id} on {source}"));
        (ability.slot, ability.definition)
    }

    fn add_temporal_archmage_timing_emblem(engine: &mut GameEngine, controller: PlayerId) {
        engine
            .state
            .static_emblems
            .push(crate::state::StaticEmblemInstance {
                object_id: u32::MAX,
                controller,
                emblem_id: "teferi_temporal_archmage_emblem".into(),
                display_name: "Teferi's Emblem".into(),
                effects: vec![StaticEmblemEffect::PlaneswalkerLoyaltyInstantTiming],
                triggered_ability: None,
            });
    }

    fn activate_command(engine: &GameEngine, source: ObjectId, ability_index: u32) -> RuledCommand {
        RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                rv1::ActivateAbility {
                    source_object_id: source,
                    ability_index,
                    source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: engine
                        .state
                        .zone_change_generation
                        .get(&source)
                        .copied()
                        .unwrap_or_default(),
                    ..Default::default()
                },
            )),
        }
    }

    #[test]
    fn self_source_loyalty_timing_is_usable_during_an_opponents_priority_window() {
        let (mut e, source, ordinary, _) = source_loyalty_timing_fixture();
        let actor = e.state.players[1].id;
        assert_eq!(e.state.active_player_id(), e.state.players[0].id);
        assert_eq!(e.state.priority_player_id(), actor);
        assert_eq!(e.state.stack.len(), 1);
        assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::Loyalty),
            3
        );

        let (source_slot, ability) = active_ability_by_id(&e, source, "plus_01");
        assert!(
            e.planeswalker_loyalty_instant_timing_allowed(source, actor, &ability),
            "the source's active static permission allows its loyalty ability at instant timing"
        );
        assert!(e.ability_activatable(source, source_slot as usize, &ability));

        let (strict_slot, strict_ability) = active_ability_by_id(&e, source, "strict_01");
        assert!(!e.planeswalker_loyalty_instant_timing_allowed(source, actor, &strict_ability));
        assert!(!e.ability_activatable(source, strict_slot as usize, &strict_ability));
        let strict_error = e
            .apply_scenario_command(actor, &activate_command(&e, source, strict_slot))
            .expect_err("the source rule does not waive explicit SorcerySpeed timing");
        assert!(strict_error
            .to_string()
            .contains("ability only at sorcery speed"));

        e.apply_scenario_command(actor, &activate_command(&e, source, source_slot))
            .expect("the source controller activates its loyalty ability over another spell");
        assert_eq!(e.state.stack.len(), 2);

        assert_eq!(e.state.objects[&ordinary].zone, Zone::Battlefield);
        assert_eq!(
            e.state.objects[&ordinary].counter_count(CounterKind::Loyalty),
            3
        );
        let (ordinary_slot, ordinary_ability) = active_ability_by_id(&e, ordinary, "plus_01");
        assert!(
            !e.ability_activatable(ordinary, ordinary_slot as usize, &ordinary_ability),
            "the self-only permission does not grant instant timing to another planeswalker"
        );
        let ordinary_error = e
            .apply_scenario_command(actor, &activate_command(&e, ordinary, ordinary_slot))
            .expect_err("the ordinary planeswalker remains sorcery timed");
        assert!(ordinary_error
            .to_string()
            .contains("ability only at sorcery speed"));
    }

    #[test]
    fn self_source_loyalty_timing_preserves_emblem_breadth_and_equip_limit() {
        let (mut e, _, ordinary, equip) = source_loyalty_timing_fixture();
        let actor = e.state.players[1].id;
        add_temporal_archmage_timing_emblem(&mut e, actor);

        let (ordinary_slot, ordinary_ability) = active_ability_by_id(&e, ordinary, "plus_01");
        assert!(e.planeswalker_loyalty_instant_timing_allowed(ordinary, actor, &ordinary_ability));
        e.apply_scenario_command(actor, &activate_command(&e, ordinary, ordinary_slot))
            .expect("the emblem still grants its controller instant timing on any walker");

        let (equip_slot, equip_ability) = active_ability_by_id(&e, equip, "equip_01");
        assert!(!e.planeswalker_loyalty_instant_timing_allowed(equip, actor, &equip_ability));
        assert!(!e.ability_activatable(equip, equip_slot as usize, &equip_ability));
        let error = e
            .apply_scenario_command(actor, &activate_command(&e, equip, equip_slot))
            .expect_err("the emblem does not waive Equip's sorcery-speed restriction");
        assert!(error.to_string().contains("ability only at sorcery speed"));
    }

    #[test]
    fn self_source_loyalty_timing_checks_actor_priority_pending_choice_and_untap_step() {
        let (mut e, source, _, _) = source_loyalty_timing_fixture();
        let actor = e.state.players[1].id;
        let other_player = e.state.players[0].id;
        let (slot, ability) = active_ability_by_id(&e, source, "plus_01");

        assert!(!e.planeswalker_loyalty_instant_timing_allowed(source, other_player, &ability));
        e.state.priority_idx = 0;
        let no_priority_error = e
            .apply_scenario_command(actor, &activate_command(&e, source, slot))
            .expect_err("the source controller cannot activate without priority");
        assert!(no_priority_error.to_string().contains("not your priority"));
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::Loyalty),
            3
        );
        assert!(e.state.activation_uses_this_turn.is_empty());

        let wrong_controller_error = e
            .apply_scenario_command(other_player, &activate_command(&e, source, slot))
            .expect_err("another player cannot activate the source's loyalty ability");
        assert!(wrong_controller_error
            .to_string()
            .contains("ability source is not in its authored zone"));
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::Loyalty),
            3
        );

        e.state.priority_idx = 1;
        e.state.pending_ability_activation = Some(rv1::PendingAbilityActivation::default());
        let no_priority_error = e
            .apply_scenario_command(actor, &activate_command(&e, source, slot))
            .expect_err("no player receives priority while an activation choice is pending");
        assert!(no_priority_error
            .to_string()
            .contains("pending ability activation"));
        e.state.pending_ability_activation = None;

        e.state.turn_step = TurnStep::Untap;
        assert!(!e.planeswalker_loyalty_instant_timing_allowed(source, actor, &ability));
        assert!(!e.ability_activatable(source, slot as usize, &ability));
        let untap_error = e
            .apply_scenario_command(actor, &activate_command(&e, source, slot))
            .expect_err("the source rule does not create priority during the untap step");
        assert!(untap_error
            .to_string()
            .contains("ability only at sorcery speed"));
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::Loyalty),
            3
        );
    }

    #[test]
    fn self_source_loyalty_timing_limit_survives_a_layer_two_control_change() {
        let (mut e, source, _, _) = source_loyalty_timing_fixture();
        let original_controller = e.state.players[1].id;
        let new_controller = e.state.players[0].id;
        let (slot, _) = active_ability_by_id(&e, source, "plus_01");
        e.apply_scenario_command(original_controller, &activate_command(&e, source, slot))
            .expect("the source controller uses its one loyalty activation");

        let usage_key = e.activation_use_key(source, ActivationUseIdentity::Loyalty);
        assert_eq!(
            e.state.activation_uses_this_turn.get(&usage_key),
            Some(&1),
            "the successful activation is recorded against this object for the turn"
        );
        e.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(new_controller),
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1_000_000,
        });
        e.reindex_battlefield_control(&mut Vec::new());
        assert_eq!(e.controller_of(source), Some(new_controller));
        assert_eq!(e.state.objects[&source].controller, new_controller);

        e.state.priority_idx = 0;
        let (new_slot, ability) = active_ability_by_id(&e, source, "plus_01");
        assert!(e.planeswalker_loyalty_instant_timing_allowed(source, new_controller, &ability));
        assert!(!e.activation_limit_allows(source, new_slot as usize, &ability));
        let error = e
            .apply_scenario_command(new_controller, &activate_command(&e, source, new_slot))
            .expect_err("changing control does not reset this permanent's turn allowance");
        assert!(error.to_string().contains("activation limit reached"));
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::Loyalty),
            4
        );
    }

    #[test]
    fn self_source_loyalty_timing_survives_a_current_type_change() {
        let (mut e, source, _, _) = source_loyalty_timing_fixture();
        let actor = e.state.players[1].id;
        e.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer4SetTypeLine(
                tricerules_card_model::TypeLineReplacement {
                    card_types: vec![PermanentTypeFilter::Artifact],
                    creature_types: Vec::new(),
                    land_types: Vec::new(),
                },
            ),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1_000_000,
        });

        let current = e
            .characteristics(source)
            .expect("the source remains on the battlefield");
        assert!(current.has_type("Artifact"));
        assert!(!current.has_type("Planeswalker"));
        let (slot, ability) = active_ability_by_id(&e, source, "plus_01");
        assert!(e
            .active_static_ability_definitions(source)
            .iter()
            .any(|definition| matches!(
                definition,
                StaticAbilityDef::SourceLoyaltyAbilitiesAtInstantTiming
            )));
        assert!(e.planeswalker_loyalty_instant_timing_allowed(source, actor, &ability));
        e.apply_scenario_command(actor, &activate_command(&e, source, slot))
            .expect("CR 606.2 keys loyalty status to its cost, not the current type line");
    }

    #[test]
    fn self_source_loyalty_timing_uses_active_definitions_after_ability_removal() {
        let (mut e, source, _, _) = source_loyalty_timing_fixture();
        let actor = e.state.players[1].id;
        let (slot, ability) = active_ability_by_id(&e, source, "plus_01");
        assert!(e.planeswalker_loyalty_instant_timing_allowed(source, actor, &ability));

        e.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1_000_000,
        });

        assert!(!e
            .active_static_ability_definitions(source)
            .iter()
            .any(|definition| matches!(
                definition,
                StaticAbilityDef::SourceLoyaltyAbilitiesAtInstantTiming
            )));
        assert!(!e.planeswalker_loyalty_instant_timing_allowed(source, actor, &ability));
        assert!(!e.ability_activatable(source, slot as usize, &ability));
        assert!(e.effective_activated_abilities(source).is_empty());
    }

    #[test]
    fn self_source_loyalty_timing_ends_after_departure_or_stale_generation() {
        let (mut e, source, _, _) = source_loyalty_timing_fixture();
        let actor = e.state.players[1].id;
        let (slot, ability) = active_ability_by_id(&e, source, "plus_01");
        let stale_command = activate_command(&e, source, slot);
        let generation = e
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or_default();
        e.state
            .zone_change_generation
            .insert(source, generation + 1);

        let stale_error = e
            .apply_scenario_command(actor, &stale_command)
            .expect_err("the previous source generation cannot activate");
        assert!(stale_error
            .to_string()
            .contains("stale ability source generation"));
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::Loyalty),
            3
        );

        super::super::resolution::move_object_to_zone(
            &mut e.state,
            e.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .expect("the source leaves through the existing zone-change funnel");
        assert!(!e.planeswalker_loyalty_instant_timing_allowed(source, actor, &ability));
        assert!(e.active_static_ability_definitions(source).is_empty());
    }
}

#[cfg(test)]
mod mana_payment_tests {
    use super::*;
    use tricerules_card_model::ActivationTiming;

    /// Build a 2-player engine and hand priority to player 0 so `pay_mana`'s priority gate passes.
    fn engine_with_priority() -> GameEngine {
        let mut e = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            1,
            &[0, 1],
            20,
        )
        .expect("new");
        e.state.priority_idx = 0;
        e
    }

    #[test]
    fn issue_499_rejects_x_on_an_activated_ability_without_x_mana_cost() {
        let mut e = engine_with_priority();
        let source = e.state.players[0].hand.remove(0);
        e.state.players[0].battlefield.push(source);
        e.state.objects.get_mut(&source).unwrap().zone = Zone::Battlefield;

        let ability = ActivatedAbilityDef {
            intrinsic_land_mana: false,
            ability_id: tricerules_card_model::AbilityId::new("activated_01").unwrap(),
            presentation: tricerules_card_model::AbilityPresentation::Fallback,
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![AbilityCost::Mana(ManaCost::parse("{1}").unwrap())],
            cost_modifiers: vec![],
            effect: vec![SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }],
            program: None,
            modal: None,
            targeting: None,
            timing: ActivationTiming::Normal,
            conditions: vec![],
            activation_limit: None,
        };
        let mut face = e.registry.get("island").unwrap().primary_face().clone();
        face.activated_abilities = vec![ability];
        let display_name = face.name.clone();
        e.state.objects.get_mut(&source).unwrap().copiable_values =
            Some(crate::state::CopiableValues {
                prototyped: false,
                source_card_id: "island".into(),
                source_face_index: 0,
                face,
                room_faces: None,
                display_name,
            });
        e.state.players[0].mana_pool.colorless = 2;
        e.reconcile_activated_ability_slots();
        let generation = e
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or_default();
        let command = rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                rv1::ActivateAbility {
                    source_object_id: source,
                    ability_index: 0,
                    source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: generation,
                    x_value: 2,
                    payment: Some(rv1::PaymentSelection {
                        mana: Some(rv1::PaymentMana {
                            c: 2,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )),
        };

        let error = e
            .apply_scenario_command(0, &command)
            .expect_err("an ability without {X} rejects an X declaration");
        assert!(error
            .to_string()
            .contains("unexpected mana X or color split"));
        assert_eq!(e.state.players[0].mana_pool.colorless, 2);
        assert!(e.state.stack.is_empty());
    }

    #[test]
    fn conditional_instant_timing_requires_the_linked_cast_cost_selection() {
        let group_id = tricerules_card_model::ChoiceId::new("cast_cost_01").unwrap();
        let option_id = tricerules_card_model::ChoiceId::new("option_01").unwrap();
        let groups = vec![CastCostGroupDef {
            group_id: group_id.clone(),
            presentation: tricerules_card_model::AbilityPresentation::Fallback,
            min: 0,
            max: 1,
            options: vec![CastCostOptionDef::Mana {
                option_id: option_id.clone(),
                presentation: tricerules_card_model::AbilityPresentation::Fallback,
                kind: tricerules_card_model::primitives::ManaCostChoiceKind::AdditionalPayment,
                cost: ManaCost::parse("{1}").unwrap(),
            }],
        }];
        let condition = CastCostReceiptCondition {
            group_id,
            option_id,
            expected_selected: true,
        };
        assert!(!command_satisfies_cast_cost_condition(
            &[],
            &groups,
            &condition,
        ));
        assert!(!command_satisfies_cast_cost_condition(
            &[rv1::CastCostGroupSelection {
                group_index: 0,
                option_index: 1,
                selected_object: None,
                expected_zone_change_generation: 0,
                battlefield_objects: None,
                repetitions: None,
            }],
            &groups,
            &condition,
        ));
        assert!(command_satisfies_cast_cost_condition(
            &[rv1::CastCostGroupSelection {
                group_index: 0,
                option_index: 0,
                selected_object: None,
                expected_zone_change_generation: 0,
                battlefield_objects: None,
                repetitions: None,
            }],
            &groups,
            &condition,
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
            provenance: crate::state::CardResultProvenance::ActionFacts,
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
    fn activation_limit_counts_follow_stable_occurrences_and_reject_unpublished_slots() {
        let mut engine = engine_with_priority();
        let object_id = engine.state.players[0].library[0];
        let ability = tricerules_cards::registry::global()
            .get("temur_devotee")
            .expect("Temur Devotee must be registered")
            .primary_face()
            .activated_abilities[0]
            .clone();
        engine.state.players[0].library.pop_front();
        engine.state.players[0].battlefield.push(object_id);
        let object = engine.state.objects.get_mut(&object_id).unwrap();
        object.card_id = "temur_devotee".into();
        object.zone = Zone::Battlefield;
        engine.reconcile_activated_ability_slots();
        let occurrence = engine.effective_activated_abilities(object_id)[0]
            .occurrence
            .clone();

        assert!(engine.activation_limit_allows(object_id, 0, &ability));
        assert!(!engine.activation_limit_allows(object_id, 1, &ability));
        let key = engine.activation_use_key(object_id, ActivationUseIdentity::Ability(occurrence));
        engine.record_limited_activations(vec![LimitedActivationUse::PerTurn(key)]);
        assert!(!engine.activation_limit_allows(object_id, 0, &ability));
        assert!(!engine.activation_limit_allows(object_id, 1, &ability));
    }

    #[test]
    fn selected_tap_payment_does_not_require_a_ready_source() {
        use rv1::cost_selection::Selection;

        let mut engine = engine_with_priority();
        let mut objects = Vec::new();
        for card_id in ["gene_pollinator", "grizzly_bears", "grizzly_bears"] {
            let oid = engine.state.players[0].library.pop_front().expect("card");
            engine.state.players[0].battlefield.push(oid);
            let object = engine.state.objects.get_mut(&oid).expect("object");
            object.card_id = card_id.into();
            object.zone = Zone::Battlefield;
            object.base_controller = 0;
            object.controller = 0;
            object.summoning_sick = true;
            objects.push(oid);
        }
        let source = objects[0];
        let mut ability = tricerules_cards::registry::global()
            .get("gene_pollinator")
            .expect("Gene Pollinator")
            .primary_face()
            .activated_abilities[0]
            .clone();
        // A mechanic-shaped two-creature payment with no source {T} component.
        ability.costs = vec![AbilityCost::TapPermanents {
            constraint: ObjectPaymentConstraint::ExactCount(2),
            filter: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..Default::default()
            },
            exclude_source: true,
        }];
        for (tapped, summoning_sick) in [(false, true), (true, false), (true, true)] {
            let object = engine.state.objects.get_mut(&source).expect("source");
            object.tapped = tapped;
            object.summoning_sick = summoning_sick;
            assert!(
                engine.ability_activatable(source, 0, &ability),
                "selected tap cost must not apply source readiness: tapped={tapped}, sick={summoning_sick}"
            );
        }
        let selections = vec![rv1::CostSelection {
            cost_index: 0,
            selection: Some(Selection::BattlefieldObjects(rv1::CostObjectRefs {
                objects: objects[1..]
                    .iter()
                    .map(|oid| rv1::CostObjectRef {
                        object_id: *oid,
                        zone_change_generation: engine
                            .state
                            .zone_change_generation
                            .get(oid)
                            .copied()
                            .unwrap_or(0),
                    })
                    .collect(),
            })),
        }];
        let plan = engine
            .plan_ability_costs(0, 0, source, &ability.costs, &[], &selections, &[], 0, 0)
            .expect("two summoning-sick creatures can pay a selected tap cost");
        engine.state.objects.get_mut(&objects[2]).unwrap().tapped = true;
        assert!(engine.commit_cost_transaction(plan).is_err());
        assert!(!engine.state.objects[&objects[1]].tapped);

        engine.state.objects.get_mut(&objects[2]).unwrap().tapped = false;
        let plan = engine
            .plan_ability_costs(0, 0, source, &ability.costs, &[], &selections, &[], 0, 0)
            .expect("valid selected tap payment");
        let receipt = engine.commit_cost_transaction(plan).expect("atomic taps");
        assert_eq!(receipt.trigger_events.len(), 2);
        assert!(objects.iter().all(|oid| engine.state.objects[oid].tapped));
    }

    #[test]
    fn combat_retained_mana_is_spent_after_ordinary_mana_of_the_same_type() {
        let mut engine = engine_with_priority();
        engine.state.players[0].mana_pool.red = 3;
        engine.state.players[0].retained_combat_mana.red = 2;

        engine.pay_generic_mana(0, 1).expect("spend ordinary red");
        assert_eq!(engine.state.players[0].mana_pool.red, 2);
        assert_eq!(engine.state.players[0].retained_combat_mana.red, 2);

        engine.pay_generic_mana(0, 1).expect("spend retained red");
        assert_eq!(engine.state.players[0].mana_pool.red, 1);
        assert_eq!(engine.state.players[0].retained_combat_mana.red, 1);
    }

    #[test]
    fn mana_ability_undo_cannot_remove_combat_retained_mana() {
        let mut engine = engine_with_priority();
        let source = engine.state.players[0].library[0];
        engine
            .state
            .objects
            .get_mut(&source)
            .expect("source")
            .tapped = true;
        engine.state.players[0].mana_pool.red = 1;
        engine.state.players[0].retained_combat_mana.red = 1;
        engine
            .state
            .undoable_mana_abilities
            .push(UndoableManaAbility {
                player: 0,
                source,
                produced: ManaAmount {
                    r: 1,
                    ..Default::default()
                },
                restriction_group_id: None,
            });

        engine
            .rewind_last_undoable_mana_ability(0, 0)
            .expect_err("ordinary mana from the activation was already spent");
        assert_eq!(engine.state.players[0].mana_pool.red, 1);
        assert_eq!(engine.state.players[0].retained_combat_mana.red, 1);
        assert_eq!(engine.state.undoable_mana_abilities.len(), 1);
        assert!(engine.state.objects[&source].tapped);
    }

    #[test]
    fn limited_activation_records_the_pre_cost_object_identity() {
        let mut engine = engine_with_priority();
        let object_id = engine.state.players[0].library[0];
        let identity = ActivationUseIdentity::Ability(ActivatedAbilityOccurrence::Printed(
            engine.ability_definition(
                object_id,
                0,
                vec![tricerules_card_model::AbilityId::new("activated_01").unwrap()],
            ),
        ));
        let key_before_costs = engine.activation_use_key(object_id, identity.clone());

        *engine
            .state
            .zone_change_generation
            .entry(object_id)
            .or_insert(0) += 1;
        engine.record_limited_activations(vec![LimitedActivationUse::PerTurn(
            key_before_costs.clone(),
        )]);

        assert_eq!(engine.state.activation_uses_this_turn[&key_before_costs], 1);
        assert_ne!(
            engine.activation_use_key(object_id, identity,),
            key_before_costs
        );
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
                count: 1,
                filter: filter.clone(),
            },
            AbilityCost::SacrificePermanent { filter, count: 1 },
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
            .plan_ability_costs(0, 0, oid, &costs, &[], &selections, &[], 0, 0)
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
                constraint: ObjectPaymentConstraint::ExactCount(2),
                filter: ZoneCardFilter {
                    exact_name: Some(namesake),
                    ..Default::default()
                },
                exclude_source: true,
            },
        ];
        let select = |ids: Vec<ObjectId>| rv1::CostSelection {
            cost_index: 1,
            selection: Some(Selection::GraveyardObjects(rv1::CostObjectRefs {
                objects: ids
                    .into_iter()
                    .map(|object_id| rv1::CostObjectRef {
                        object_id,
                        zone_change_generation: e
                            .state
                            .zone_change_generation
                            .get(&object_id)
                            .copied()
                            .unwrap_or(0),
                    })
                    .collect(),
            })),
        };

        let duplicate = [select(vec![cards[1], cards[1]])];
        assert!(e
            .plan_ability_costs(0, 0, source, &costs, &[], &duplicate, &[], 0, 0)
            .is_err());
        assert!(cards
            .iter()
            .all(|oid| e.state.objects[oid].zone == Zone::Graveyard));

        let selected = [select(vec![cards[1], cards[2]])];
        let plan = e
            .plan_ability_costs(0, 0, source, &costs, &[], &selected, &[], 0, 0)
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
            excluded_objects: vec![tricerules_card_model::TargetObjectExclusion::Source],
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

    #[test]
    fn static_grant_owner_is_frozen_before_its_source_leaves_and_returns() {
        const GRANTOR: &str = r#"(id: "program_grantor", name: "Program Grantor", face_id: "program_grantor",
                types: ["Artifact"],
                static_abilities: [(
                    ability_id: "static_01", presentation: Fallback,
                    definition: GrantActivatedAbilityToPermanents(
                        filter: (kind: Creature, controller: You),
                        activated_abilities: [(
                            ability_id: "activated_01", presentation: Fallback,
                            costs: [SacrificePermanent(filter: (kind: AnyPermanent, controller: You, permanent_types: [Artifact]))],
                            effect: [GainLife(amount: 1)],
                        )],
                    ),
                )],
            )"#;
        const RECIPIENT: &str = r#"(id: "program_recipient", name: "Program Recipient", face_id: "program_recipient",
            types: ["Creature"], power: 2, toughness: 2)"#;
        let registry = CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/forest.ron"),
                GRANTOR,
                RECIPIENT,
            ],
            &[],
        )
        .expect("static grant fixture is valid");
        let mut e = engine_with_priority();
        e.registry = Box::leak(Box::new(registry));
        let place = |engine: &mut GameEngine, card: &str| {
            engine
                .apply_dev_command(
                    0,
                    &rv1::DevCommand {
                        target_player_id: 0,
                        dev: Some(rv1::dev_command::Dev::PutCardInZone(
                            rv1::DevPutCardInZone {
                                card_name: card.into(),
                                zone: rv1::DevZone::Battlefield as i32,
                                ready: true,
                            },
                        )),
                    },
                )
                .expect("put fixture permanent on battlefield");
            engine.state.next_object_id - 1
        };
        let grantor = place(&mut e, "Program Grantor");
        let recipient = place(&mut e, "Program Recipient");
        e.reconcile_activated_ability_slots();

        let effective = e
            .effective_activated_abilities(recipient)
            .into_iter()
            .find(|ability| matches!(ability.occurrence, ActivatedAbilityOccurrence::Granted(_)))
            .expect("grantor supplies the recipient's activated ability");
        let ActivatedAbilityOccurrence::Granted(TriggerAbilityOrigin::StaticGrant {
            source_id,
            source_zone_change,
            definition,
        }) = effective.occurrence.clone()
        else {
            panic!("the announced occurrence is the authored static grant");
        };
        assert_eq!(source_id, grantor);
        let exact_owner = tricerules_card_model::ProgramOwnerId::ability(definition.clone());
        let batch = e
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                        rv1::ActivateAbility {
                            source_object_id: recipient,
                            source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                            ability_index: effective.slot,
                            expected_zone_change_generation: e
                                .state
                                .zone_change_generation
                                .get(&recipient)
                                .copied()
                                .unwrap_or_default(),
                            cost_selections: vec![rv1::CostSelection {
                                cost_index: 0,
                                selection: Some(rv1::cost_selection::Selection::PermanentId(
                                    grantor,
                                )),
                            }],
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("the granted ability can sacrifice its grantor and resolve later");
        let frozen_occurrence = {
            let stack_item = e
                .state
                .stack
                .last()
                .expect("activated ability is on the stack");
            assert_eq!(stack_item.card_id, "program_recipient");
            assert_eq!(
                stack_item.ability_program_owner(),
                Some(exact_owner.clone())
            );
            assert_eq!(
                stack_item.activated_ability_occurrence,
                Some(effective.occurrence.clone()),
                "the full grant source incarnation and nested path survive payment",
            );
            let copied_stack_item = stack_item.clone();
            assert_eq!(
                copied_stack_item.activated_ability_occurrence,
                Some(effective.occurrence.clone()),
                "stack copies retain the bound occurrence",
            );
            assert_eq!(
                copied_stack_item.ability_program_owner(),
                Some(exact_owner.clone()),
                "stack copies keep the exact program owner",
            );
            stack_item.activated_ability_occurrence.clone()
        };
        assert_eq!(e.state.objects[&grantor].zone, Zone::Graveyard);

        super::super::resolution::move_object_to_zone(
            &mut e.state,
            e.registry,
            grantor,
            Zone::Battlefield,
            Some(0),
        )
        .expect("move the same grantor object back to the battlefield");
        e.emit_static_abilities_on_enter(grantor);
        e.reconcile_activated_ability_slots();
        let returned_occurrence = e
            .effective_activated_abilities(recipient)
            .into_iter()
            .find(|ability| matches!(ability.occurrence, ActivatedAbilityOccurrence::Granted(_)))
            .expect("the returned grantor supplies a fresh ability occurrence")
            .occurrence;
        assert_ne!(returned_occurrence, effective.occurrence);
        assert_eq!(
            e.state.stack.last().unwrap().ability_program_owner(),
            Some(exact_owner),
            "the old stack item remains bound to its original owner after a return",
        );
        assert!(batch
            .events
            .iter()
            .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::StackPushed(_)))));
        let Some(ActivatedAbilityOccurrence::Granted(TriggerAbilityOrigin::StaticGrant {
            source_zone_change: captured_generation,
            ..
        })) = frozen_occurrence
        else {
            panic!("stack item keeps the original static grant occurrence");
        };
        assert_eq!(captured_generation, source_zone_change);
        assert_ne!(
            e.state.zone_change_generation[&grantor], captured_generation,
            "returning grantor is a new source incarnation",
        );

        let mut resolving_ability = effective.definition.clone();
        resolving_ability.costs.clear();
        e.state.add_activated_ability_grant(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(recipient),
            kind: ContinuousEffectKind::GrantActivatedAbility(Box::new(resolving_ability)),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: e.state.command_index + 1,
        });
        e.reconcile_activated_ability_slots();
        let resolving_grant = e
            .effective_activated_abilities(recipient)
            .into_iter()
            .find(|ability| {
                matches!(
                    ability.occurrence,
                    ActivatedAbilityOccurrence::Granted(
                        TriggerAbilityOrigin::ResolvingGrant { .. },
                    )
                )
            })
            .expect("resolving grants remain live legacy activated abilities");
        let resolving_batch = e
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                        rv1::ActivateAbility {
                            source_object_id: recipient,
                            source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                            ability_index: resolving_grant.slot,
                            expected_zone_change_generation: e
                                .state
                                .zone_change_generation
                                .get(&recipient)
                                .copied()
                                .unwrap_or_default(),
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("a resolving grant can still be activated and put on the stack");
        let resolving_item = e
            .state
            .stack
            .last()
            .expect("resolving grant is on the stack");
        assert_eq!(
            resolving_item.activated_ability_occurrence,
            Some(resolving_grant.occurrence)
        );
        assert_eq!(
            resolving_item.ability_program_owner(),
            None,
            "a runtime grant without an authored owner does not gain a guessed program identity",
        );
        assert!(resolving_batch
            .events
            .iter()
            .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::StackPushed(_)))));
        let life_before = e.state.players[0].life;
        e.resolve_top_of_stack(&mut Vec::new())
            .expect("legacy resolving-grant effect remains executable");
        assert_eq!(e.state.players[0].life, life_before + 1);
    }
}
