mod attacking;
mod blocking;
use blocking::BlockGraph;

use super::damage::{DamageEvent, DamageRecipient};
use super::events::{ev_log, ev_phase, ev_priority_changed, object_display_name};
use super::legal_actions::fill_legal;
use super::*;

#[derive(Clone)]
pub(super) struct PendingAttackDeclarationInternal {
    pub assignments: HashMap<ObjectId, CombatAttackAssignment>,
    /// Taps caused by CR 508.1f; kept so cancellation can remove just this action's contribution.
    pub tapped_attackers: Vec<TriggerObjectRef>,
    /// Tap-trigger matches captured at event time with their trigger-use limits already reserved.
    pub tap_triggers: Vec<super::triggers::CollectedTrigger>,
    /// Trigger state before CR 508.1f, used to remove only the canceled declaration's triggers.
    pub pre_attack_state: GameState,
    /// State at the start of the payment window, including the chosen attackers already tapped.
    pub payment_window_base: GameState,
    /// Every accepted mana activation during this declaration, with an event-time state and cost
    /// receipt so CR 733.1 can reverse the exact action without re-running its ability.
    pub mana_ability_receipts: Vec<AttackManaAbilityReceipt>,
}

#[derive(Clone)]
pub(super) struct AttackManaAbilityReceipt {
    pub activation_command_index: u64,
    pub source: TriggerObjectRef,
    pub ability_path: Vec<tricerules_cards::AbilityId>,
    pub source_label: String,
    pub ability: ActivatedAbilityDef,
    pub before_state: GameState,
    pub cost_plan: super::payment::transaction::CostTransactionPlan,
    pub cost_payment: super::payment::transaction::CostPaymentReceipt,
    /// Unrestricted and restricted mana from earlier receipts that paid this activation's cost.
    pub mana_sources_consumed: Vec<ManaSourceConsumption>,
    pub activation_uses: Vec<super::casting::LimitedActivationUse>,
    pub mana_damage: Option<super::damage::DamageSpec>,
    /// The exact post-prevention damage result produced by the activation. A retained receipt
    /// applies this result and its consumed finite-shield debits without running prevention again.
    pub mana_damage_results: Vec<super::damage::CompletedDamage>,
    /// Event-time trigger matches caused by the completed damage and lifelink events.
    pub mana_damage_triggers: Vec<super::triggers::CollectedTrigger>,
    pub mana_output: ManaAmount,
    pub restriction_group_id: Option<u32>,
    pub mana_restriction: Option<tricerules_cards::ManaSpendingRestriction>,
    pub restriction_presentation: Option<rv1::PresentationRef>,
    /// Event-time trigger matches from paying this activation's costs, before use-limit filtering.
    pub cost_triggers: Vec<super::triggers::CollectedTrigger>,
    pub simple_tap: bool,
    /// Library movement/reveal/shuffle makes reversal unavailable under CR 733.1. Other
    /// currently supported mana actions have complete state/cost snapshots above.
    pub reversible: bool,
}

/// Event-time values captured from one accepted activated mana ability before receipt journaling.
pub(super) struct AttackManaAbilityReceiptInput {
    pub before_state: GameState,
    pub source: ObjectId,
    pub ability_path: Vec<tricerules_cards::AbilityId>,
    pub source_label: String,
    pub ability: ActivatedAbilityDef,
    pub cost_plan: super::payment::transaction::CostTransactionPlan,
    pub payment: super::payment::transaction::CostPaymentReceipt,
    pub mana_output: ManaAmount,
    pub restriction_group_id: Option<u32>,
    pub activation_uses: Vec<super::casting::LimitedActivationUse>,
    pub mana_damage: Option<super::damage::DamageSpec>,
    pub mana_damage_results: Vec<super::damage::CompletedDamage>,
    pub mana_damage_triggers: Vec<super::triggers::CollectedTrigger>,
    pub cost_triggers: Vec<super::triggers::CollectedTrigger>,
    pub has_cost_triggers: bool,
}

struct PendingAttackTaxPaymentStart<'a> {
    attacking_player: PlayerId,
    assignments: &'a [rv1::AttackAssignment],
    parsed_assignments: HashMap<ObjectId, CombatAttackAssignment>,
    tapping_attackers: &'a [ObjectId],
    tap_events: &'a [GameEvent],
    pre_attack_state: GameState,
    generic_mana_cost: u32,
}

#[derive(Clone)]
pub(super) struct ManaSourceConsumption {
    pub activation_command_index: u64,
    pub amount: ManaAmount,
}

struct ReappliedAttackManaReceipt {
    cost_plan: super::payment::transaction::CostTransactionPlan,
    payment: super::payment::transaction::CostPaymentReceipt,
    restriction_group_id: Option<u32>,
    mana_sources_consumed: Vec<ManaSourceConsumption>,
    events: Vec<rv1::RuledEvent>,
}

fn mana_amount_add(left: ManaAmount, right: ManaAmount) -> ManaAmount {
    ManaAmount {
        w: left.w.saturating_add(right.w),
        u: left.u.saturating_add(right.u),
        b: left.b.saturating_add(right.b),
        r: left.r.saturating_add(right.r),
        g: left.g.saturating_add(right.g),
        c: left.c.saturating_add(right.c),
    }
}

fn mana_amount_sub(left: ManaAmount, right: ManaAmount) -> ManaAmount {
    ManaAmount {
        w: left.w.saturating_sub(right.w),
        u: left.u.saturating_sub(right.u),
        b: left.b.saturating_sub(right.b),
        r: left.r.saturating_sub(right.r),
        g: left.g.saturating_sub(right.g),
        c: left.c.saturating_sub(right.c),
    }
}

/// Consume a requested amount from an available bag and return what could not be covered.
fn take_mana_amount(available: &mut ManaAmount, requested: ManaAmount) -> ManaAmount {
    let taken = ManaAmount {
        w: available.w.min(requested.w),
        u: available.u.min(requested.u),
        b: available.b.min(requested.b),
        r: available.r.min(requested.r),
        g: available.g.min(requested.g),
        c: available.c.min(requested.c),
    };
    *available = mana_amount_sub(*available, taken);
    mana_amount_sub(requested, taken)
}

fn mana_amount_is_empty(amount: ManaAmount) -> bool {
    amount == ManaAmount::default()
}

fn record_mana_source_consumption(
    consumptions: &mut Vec<ManaSourceConsumption>,
    activation_command_index: u64,
    amount: ManaAmount,
) {
    if mana_amount_is_empty(amount) {
        return;
    }
    if let Some(existing) = consumptions
        .iter_mut()
        .find(|source| source.activation_command_index == activation_command_index)
    {
        existing.amount = mana_amount_add(existing.amount, amount);
    } else {
        consumptions.push(ManaSourceConsumption {
            activation_command_index,
            amount,
        });
    }
}

fn receipt_unspent_mana_output(
    receipts: &[AttackManaAbilityReceipt],
    source_index: usize,
) -> ManaAmount {
    let source = &receipts[source_index];
    let spent = receipts[source_index + 1..]
        .iter()
        .flat_map(|receipt| receipt.mana_sources_consumed.iter())
        .filter(|consumption| {
            consumption.activation_command_index == source.activation_command_index
        })
        .fold(ManaAmount::default(), |total, consumption| {
            mana_amount_add(total, consumption.amount)
        });
    mana_amount_sub(source.mana_output, spent)
}

/// Mana pools do not distinguish sources with identical color and restriction. For CR 733.1,
/// attribute costs deterministically: pre-window pool contributions first, then earlier mana
/// ability receipts in activation order. Restricted mana is partitioned by its spending group.
fn mana_sources_consumed_by_cost(
    receipts: &[AttackManaAbilityReceipt],
    state: &GameState,
    payer: PlayerId,
    cost_plan: &super::payment::transaction::CostTransactionPlan,
) -> Vec<ManaSourceConsumption> {
    let Some(player_index) = state.player_idx(payer) else {
        return Vec::new();
    };
    let Some(player_state) = state.players.get(player_index) else {
        return Vec::new();
    };
    let (unrestricted_spent, restricted_spent) = cost_plan.mana_source_spend();
    let mut sources_consumed = Vec::new();

    let unrestricted_sources = receipts
        .iter()
        .enumerate()
        .filter(|(_, receipt)| receipt.restriction_group_id.is_none())
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let unrestricted_output = unrestricted_sources
        .iter()
        .fold(ManaAmount::default(), |total, index| {
            mana_amount_add(total, receipt_unspent_mana_output(receipts, *index))
        });
    let mut ordinary_pool = mana_amount_sub(
        ManaAmount {
            w: player_state.mana_pool.white,
            u: player_state.mana_pool.blue,
            b: player_state.mana_pool.black,
            r: player_state.mana_pool.red,
            g: player_state.mana_pool.green,
            c: player_state.mana_pool.colorless,
        },
        unrestricted_output,
    );
    let mut unallocated = take_mana_amount(&mut ordinary_pool, unrestricted_spent);
    for source_index in unrestricted_sources {
        let mut source_output = receipt_unspent_mana_output(receipts, source_index);
        let remaining = take_mana_amount(&mut source_output, unallocated);
        let consumed = mana_amount_sub(unallocated, remaining);
        record_mana_source_consumption(
            &mut sources_consumed,
            receipts[source_index].activation_command_index,
            consumed,
        );
        unallocated = remaining;
        if mana_amount_is_empty(unallocated) {
            break;
        }
    }
    debug_assert!(
        mana_amount_is_empty(unallocated),
        "accepted unrestricted mana cost must have a source lineage"
    );

    for (group_id, group_spent) in restricted_spent {
        let group_sources = receipts
            .iter()
            .enumerate()
            .filter(|(_, receipt)| receipt.restriction_group_id == Some(group_id))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let group_output = group_sources
            .iter()
            .fold(ManaAmount::default(), |total, index| {
                mana_amount_add(total, receipt_unspent_mana_output(receipts, *index))
            });
        let group_available = player_state
            .restricted_mana
            .iter()
            .filter(|contribution| contribution.restriction_group_id == group_id)
            .fold(ManaAmount::default(), |total, contribution| {
                mana_amount_add(total, contribution.amount)
            });
        let mut group_background = mana_amount_sub(group_available, group_output);
        let mut group_unallocated = take_mana_amount(&mut group_background, group_spent);
        for source_index in group_sources {
            let mut source_output = receipt_unspent_mana_output(receipts, source_index);
            let remaining = take_mana_amount(&mut source_output, group_unallocated);
            let consumed = mana_amount_sub(group_unallocated, remaining);
            record_mana_source_consumption(
                &mut sources_consumed,
                receipts[source_index].activation_command_index,
                consumed,
            );
            group_unallocated = remaining;
            if mana_amount_is_empty(group_unallocated) {
                break;
            }
        }
        debug_assert!(
            mana_amount_is_empty(group_unallocated),
            "accepted restricted mana cost must have a source lineage"
        );
    }
    sources_consumed
}

impl GameEngine {
    /// Add the generic attack cost contributed by active battlefield statics controlled by a
    /// directly attacked player. A planeswalker or Battle edge is never taxed by this primitive.
    pub(super) fn attack_tax_per_attacker(&self, defender: CombatDefenderTarget) -> u128 {
        let CombatDefenderTarget::Player(defending_player) = defender else {
            return 0;
        };
        let mut total = 0u128;
        for object in self.state.objects.values() {
            if object.zone != Zone::Battlefield
                || self.controller_of(object.id) != Some(defending_player)
            {
                continue;
            }
            for ability in self.active_static_ability_definitions(object.id) {
                if let StaticAbilityDef::AttackTax {
                    generic_per_attacker,
                } = ability
                {
                    total = total.saturating_add(u128::from(generic_per_attacker));
                }
            }
        }
        total
    }

    fn attack_tax_total(
        &self,
        assignments: &HashMap<ObjectId, CombatAttackAssignment>,
    ) -> Result<u32, EngineError> {
        assignments.values().try_fold(0u32, |total, assignment| {
            let per_attacker = self.attack_tax_per_attacker(assignment.defender);
            let per_attacker = u32::try_from(per_attacker)
                .map_err(|_| EngineError::Illegal("attack tax exceeds numeric limit"))?;
            total
                .checked_add(per_attacker)
                .ok_or(EngineError::Illegal("attack tax exceeds numeric limit"))
        })
    }

    fn begin_attack_tax_payment(
        &mut self,
        start: PendingAttackTaxPaymentStart<'_>,
    ) -> Result<RuledEventBatch, EngineError> {
        let PendingAttackTaxPaymentStart {
            attacking_player,
            assignments,
            parsed_assignments,
            tapping_attackers,
            tap_events,
            pre_attack_state,
            generic_mana_cost,
        } = start;
        let transaction_id = self.state.next_attack_declaration_transaction_id;
        self.state.next_attack_declaration_transaction_id = transaction_id.saturating_add(1);
        let pending = rv1::PendingAttackDeclaration {
            transaction_id,
            revision: 1,
            attacking_player_id: attacking_player,
            assignments: assignments.to_vec(),
            generic_mana_cost,
            ..Default::default()
        };
        self.state.pending_attack_declaration = Some(pending.clone());
        let tap_triggers = self.collect_event_triggers(tap_events);
        let tap_triggers = self.reserve_trigger_uses(tap_triggers);
        self.pending_attack_declaration_internal = Some(PendingAttackDeclarationInternal {
            assignments: parsed_assignments,
            tapped_attackers: tapping_attackers
                .iter()
                .filter_map(|object_id| self.trigger_object_ref(*object_id))
                .collect(),
            tap_triggers,
            pre_attack_state,
            payment_window_base: self.state.clone(),
            mana_ability_receipts: Vec::new(),
        });

        // The chosen creatures have been tapped, but are not yet attackers. Trigger snapshots
        // remain parked until the attack transaction either commits or is reversed.
        let mut batch = RuledEventBatch::default();
        batch.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::AttackPaymentRequired(
                rv1::AttackPaymentRequired {
                    pending: Some(pending),
                },
            )),
        });
        batch.events.push(ev_log(format!(
            "P{attacking_player} must pay {{{generic_mana_cost}}} to complete the attack declaration"
        )));
        Ok(batch)
    }

    pub(super) fn record_attack_mana_ability(&mut self, input: AttackManaAbilityReceiptInput) {
        let AttackManaAbilityReceiptInput {
            before_state,
            source,
            ability_path,
            source_label,
            ability,
            cost_plan,
            payment,
            mana_output,
            restriction_group_id,
            activation_uses,
            mana_damage,
            mana_damage_results,
            mana_damage_triggers,
            cost_triggers,
            has_cost_triggers,
        } = input;
        let Some(payer) = before_state
            .pending_attack_declaration
            .as_ref()
            .map(|pending| pending.attacking_player_id)
        else {
            return;
        };
        let mana_sources_consumed = self
            .pending_attack_declaration_internal
            .as_ref()
            .map(|internal| {
                mana_sources_consumed_by_cost(
                    &internal.mana_ability_receipts,
                    &before_state,
                    payer,
                    &cost_plan,
                )
            })
            .unwrap_or_default();
        let Some(internal) = self.pending_attack_declaration_internal.as_mut() else {
            return;
        };
        let Some(source_object) = before_state.objects.get(&source) else {
            return;
        };
        let source_zone_change_generation = before_state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let source_ref = TriggerObjectRef {
            object_id: source,
            zone_change_generation: source_zone_change_generation,
            controller_at_event: source_object.controller,
        };
        let library_action = payment.move_events.iter().any(|event| {
            matches!(
                &event.ev,
                Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                    if moved.destination == rv1::permanent_moved::Destination::Library as i32
            )
        });
        let simple_tap = ability.costs.as_slice() == [AbilityCost::Tap]
            && ability.mana_ability_damage_to_controller().is_none()
            && !has_cost_triggers
            && activation_uses.is_empty();
        let mana_restriction = ability.mana_restriction().cloned();
        let restriction_presentation = restriction_group_id
            .and_then(|group_id| group_id.checked_sub(1))
            .and_then(|index| {
                self.state
                    .mana_restriction_presentations
                    .get(index as usize)
            })
            .cloned()
            .flatten();
        internal
            .mana_ability_receipts
            .push(AttackManaAbilityReceipt {
                activation_command_index: self.state.command_index,
                source: source_ref,
                ability_path,
                source_label,
                ability,
                before_state,
                cost_plan,
                cost_payment: payment,
                mana_sources_consumed,
                activation_uses,
                mana_damage,
                mana_damage_results,
                mana_damage_triggers,
                mana_output,
                restriction_group_id,
                mana_restriction,
                restriction_presentation,
                cost_triggers,
                simple_tap,
                reversible: !library_action,
            });
    }

    pub(super) fn record_attack_mana_damage_results(
        &mut self,
        player: PlayerId,
        source_object_id: ObjectId,
        completed: &[super::damage::CompletedDamage],
        event_time_triggers: Vec<super::triggers::CollectedTrigger>,
    ) {
        let Some(pending) = self.state.pending_attack_declaration.as_ref() else {
            return;
        };
        if pending.attacking_player_id != player {
            return;
        }
        let Some(receipt) =
            self.pending_attack_declaration_internal
                .as_mut()
                .and_then(|internal| {
                    internal
                        .mana_ability_receipts
                        .iter_mut()
                        .rev()
                        .find(|receipt| {
                            receipt.source.object_id == source_object_id
                                && receipt.mana_damage.is_some()
                                && receipt.mana_damage_results.is_empty()
                        })
                })
        else {
            return;
        };
        receipt.mana_damage_results = completed.to_vec();
        receipt.mana_damage_triggers = event_time_triggers;
    }

    fn attack_mana_receipt_unavailable_reason(
        &self,
        receipts: &[AttackManaAbilityReceipt],
        index: usize,
    ) -> Option<&'static str> {
        let Some(receipt) = receipts.get(index) else {
            return Some("This attack mana receipt is unavailable.");
        };
        let contains_library_move = receipt.cost_payment.move_events.iter().any(|event| {
            matches!(
                &event.ev,
                Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                    if moved.destination == rv1::permanent_moved::Destination::Library as i32
            )
        });
        if !receipt.reversible || contains_library_move {
            return Some(
                "CR 733.1 prohibits reversing a mana ability that moved a card from a library.",
            );
        }
        if receipt.ability_path.last() != Some(&receipt.ability.ability_id) {
            return Some("This attack mana receipt no longer matches its activated ability.");
        }
        if receipts[index + 1..].iter().any(|later| {
            later.mana_sources_consumed.iter().any(|consumption| {
                consumption.activation_command_index == receipt.activation_command_index
            })
        }) {
            return Some("Mana from this activation was spent on a later mana ability.");
        }
        if index + 1 == receipts.len() {
            return None;
        }
        // Rebase the exact later cost receipts over the selected activation's pre-state. This
        // keeps independent activations, including sacrifice/counter/life costs, while detecting
        // a true dependency such as a later cost that spent the selected activation's only mana.
        let mut candidate = self.clone();
        candidate.state = receipt.before_state.clone();
        let Some(player) = candidate
            .state
            .pending_attack_declaration
            .as_ref()
            .map(|pending| pending.attacking_player_id)
        else {
            return Some("The attack payment transaction is unavailable.");
        };
        let mut restriction_groups = HashMap::new();
        let mut preceding_receipts = receipts[..index].to_vec();
        for later in &receipts[index + 1..] {
            let before_state = candidate.state.clone();
            let Ok(reapplied) = candidate.reapply_attack_mana_receipt(
                player,
                later,
                &mut restriction_groups,
                &preceding_receipts,
            ) else {
                return Some("A later mana ability depends on this activation's source or costs.");
            };
            let mut rebased = later.clone();
            rebased.before_state = before_state;
            rebased.cost_plan = reapplied.cost_plan;
            rebased.cost_payment = reapplied.payment;
            rebased.restriction_group_id = reapplied.restriction_group_id;
            rebased.mana_sources_consumed = reapplied.mana_sources_consumed;
            preceding_receipts.push(rebased);
        }
        None
    }

    fn attack_mana_receipt_is_reversible(
        &self,
        receipts: &[AttackManaAbilityReceipt],
        index: usize,
    ) -> bool {
        self.attack_mana_receipt_unavailable_reason(receipts, index)
            .is_none()
    }

    pub(super) fn attack_mana_ability_undo_options(
        &self,
        player: PlayerId,
    ) -> Vec<rv1::AttackManaAbilityUndoOption> {
        if self
            .state
            .pending_attack_declaration
            .as_ref()
            .filter(|pending| pending.attacking_player_id == player)
            .is_none()
        {
            return Vec::new();
        }
        let Some(internal) = self.pending_attack_declaration_internal.as_ref() else {
            return Vec::new();
        };
        internal
            .mana_ability_receipts
            .iter()
            .enumerate()
            .map(|(index, receipt)| {
                let unavailable_reason = self
                    .attack_mana_receipt_unavailable_reason(&internal.mana_ability_receipts, index);
                let reversible = unavailable_reason.is_none();
                rv1::AttackManaAbilityUndoOption {
                    activation_command_index: receipt.activation_command_index,
                    source_object_id: receipt.source.object_id,
                    source_label: receipt.source_label.clone(),
                    reversible,
                    unavailable_reason: unavailable_reason.unwrap_or_default().into(),
                }
            })
            .collect()
    }

    pub(super) fn undo_attack_mana_ability(
        &mut self,
        player: PlayerId,
        command: &rv1::UndoManaAbility,
    ) -> Result<RuledEventBatch, EngineError> {
        let pending = self
            .state
            .pending_attack_declaration
            .as_ref()
            .ok_or(EngineError::Illegal("no attack payment is pending"))?;
        if pending.attacking_player_id != player
            || command.attack_transaction_id != pending.transaction_id
            || command.activation_command_index == 0
        {
            return Err(EngineError::Illegal("wrong or stale attack mana receipt"));
        }
        let baseline_matches = self
            .pending_attack_declaration_internal
            .as_ref()
            .and_then(|internal| {
                internal
                    .payment_window_base
                    .pending_attack_declaration
                    .as_ref()
            })
            .is_some_and(|baseline| baseline.transaction_id == pending.transaction_id);
        if !baseline_matches {
            return Err(EngineError::Illegal("attack payment baseline is stale"));
        }
        let (receipt, prefix, suffix) = {
            let internal = self
                .pending_attack_declaration_internal
                .as_ref()
                .ok_or(EngineError::Illegal("missing attack declaration snapshot"))?;
            let index = internal
                .mana_ability_receipts
                .iter()
                .position(|receipt| {
                    receipt.activation_command_index == command.activation_command_index
                })
                .ok_or(EngineError::Illegal("unknown attack mana receipt"))?;
            if !self.attack_mana_receipt_is_reversible(&internal.mana_ability_receipts, index) {
                return Err(EngineError::Illegal(
                    "attack mana receipt is not reversible",
                ));
            }
            (
                internal.mana_ability_receipts[index].clone(),
                internal.mana_ability_receipts[..index].to_vec(),
                internal.mana_ability_receipts[index + 1..].to_vec(),
            )
        };
        let after_state = self.state.clone();
        let mut batch = RuledEventBatch::default();
        {
            // The exact pre-activation snapshot reverses its costs, output, damage, trigger
            // reservations and source changes without replaying the accepted mana ability.
            for (object_id, before_object) in &receipt.before_state.objects {
                let Some(after_object) = after_state.objects.get(object_id) else {
                    continue;
                };
                if before_object.zone != after_object.zone {
                    let destination = match before_object.zone {
                        Zone::Hand => rv1::permanent_moved::Destination::Hand,
                        Zone::Battlefield => rv1::permanent_moved::Destination::Battlefield,
                        Zone::Graveyard => rv1::permanent_moved::Destination::Graveyard,
                        Zone::Library => rv1::permanent_moved::Destination::Library,
                        Zone::Exile => rv1::permanent_moved::Destination::Exile,
                        Zone::Command => rv1::permanent_moved::Destination::Command,
                        Zone::Stack => rv1::permanent_moved::Destination::Unspecified,
                    };
                    batch.events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::PermanentMoved(rv1::PermanentMoved {
                            object_id: *object_id,
                            owner_player_id: before_object.owner,
                            destination: destination as i32,
                            card_id: before_object.card_id.clone(),
                            controller_player_id: before_object.controller,
                            face_down: before_object.face_down,
                            ..Default::default()
                        })),
                    });
                } else if before_object.zone == Zone::Battlefield
                    && after_object.tapped
                    && !before_object.tapped
                {
                    self.state.untapped_this_command.push(*object_id);
                }
            }
            for (before_player, after_player) in receipt
                .before_state
                .players
                .iter()
                .zip(after_state.players.iter())
            {
                let delta = before_player.life.saturating_sub(after_player.life);
                if delta != 0 {
                    batch.events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                            player_id: before_player.id,
                            new_total: before_player.life,
                            delta,
                        })),
                    });
                }
            }
            let command_index = after_state.command_index;
            let next_object_id = after_state
                .next_object_id
                .max(receipt.before_state.next_object_id);
            let next_trigger_grant_id = after_state
                .next_trigger_grant_id
                .max(receipt.before_state.next_trigger_grant_id);
            let next_tap_action_id = after_state
                .next_tap_action_id
                .max(receipt.before_state.next_tap_action_id);
            let next_game_rule_timestamp = after_state
                .next_game_rule_timestamp
                .max(receipt.before_state.next_game_rule_timestamp);
            self.state = receipt.before_state.clone();
            self.state.command_index = command_index;
            self.state.next_object_id = next_object_id;
            self.state.next_trigger_grant_id = next_trigger_grant_id;
            self.state.next_tap_action_id = next_tap_action_id;
            self.state.next_game_rule_timestamp = next_game_rule_timestamp;
            let mut restriction_groups = HashMap::new();
            let mut kept_receipts = prefix;
            for later in &suffix {
                let before_state = self.state.clone();
                let reapplied = self.reapply_attack_mana_receipt(
                    player,
                    later,
                    &mut restriction_groups,
                    &kept_receipts,
                )?;
                batch.events.extend(reapplied.events);
                let mut kept = later.clone();
                kept.before_state = before_state;
                kept.cost_plan = reapplied.cost_plan;
                kept.cost_payment = reapplied.payment;
                kept.restriction_group_id = reapplied.restriction_group_id;
                kept.mana_sources_consumed = reapplied.mana_sources_consumed;
                kept_receipts.push(kept);
            }
            self.pending_attack_declaration_internal
                .as_mut()
                .expect("pending attack transaction")
                .mana_ability_receipts = kept_receipts;
        }
        batch.events.push(ev_log(format!(
            "P{player} undoes mana ability: {}",
            receipt.source_label
        )));
        if let Some(pending) = self.state.pending_attack_declaration.clone() {
            batch.events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::AttackPaymentRequired(
                    rv1::AttackPaymentRequired {
                        pending: Some(pending),
                    },
                )),
            });
        }
        Ok(batch)
    }

    /// Reapply a retained receipt's exact costs and recorded outputs after a preceding receipt was
    /// reversed. This does not execute the mana ability or resolve a new prevention sequence.
    fn reapply_attack_mana_receipt(
        &mut self,
        player: PlayerId,
        receipt: &AttackManaAbilityReceipt,
        restriction_group_remap: &mut HashMap<u32, u32>,
        preceding_receipts: &[AttackManaAbilityReceipt],
    ) -> Result<ReappliedAttackManaReceipt, EngineError> {
        if self
            .state
            .zone_change_generation
            .get(&receipt.source.object_id)
            .copied()
            .unwrap_or(0)
            != receipt.source.zone_change_generation
            || self
                .state
                .objects
                .get(&receipt.source.object_id)
                .is_none_or(|object| {
                    object.zone != Zone::Battlefield || object.controller != player
                })
        {
            return Err(EngineError::Illegal(
                "later mana activation depends on the reversed receipt's source state",
            ));
        }
        let mut cost_plan = receipt.cost_plan.clone();
        cost_plan.remap_restricted_groups(restriction_group_remap);
        let cost_plan = cost_plan.rebase_for_attack_receipt(&self.state)?;
        let mana_sources_consumed =
            mana_sources_consumed_by_cost(preceding_receipts, &self.state, player, &cost_plan);
        let cost_plan = self.prepare_cost_transaction_commit(cost_plan)?;
        let payment = self.commit_prevalidated_cost_transaction(cost_plan.clone())?;
        self.record_limited_activations(receipt.activation_uses.clone());

        let restriction_group_id = if let Some(restriction) = &receipt.mana_restriction {
            let group_id = if let Some(index) = self
                .state
                .mana_restrictions
                .iter()
                .position(|candidate| candidate == restriction)
            {
                index as u32 + 1
            } else {
                self.state.mana_restrictions.push(restriction.clone());
                self.state
                    .mana_restriction_presentations
                    .push(receipt.restriction_presentation.clone());
                self.state.mana_restrictions.len() as u32
            };
            Some(group_id)
        } else {
            None
        };
        let player_index = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        if let Some(group_id) = restriction_group_id {
            self.state.players[player_index].restricted_mana.push(
                crate::state::RestrictedManaContribution {
                    restriction_group_id: group_id,
                    amount: receipt.mana_output,
                },
            );
        } else {
            let pool = &mut self.state.players[player_index].mana_pool;
            pool.white = pool.white.saturating_add(receipt.mana_output.w);
            pool.blue = pool.blue.saturating_add(receipt.mana_output.u);
            pool.black = pool.black.saturating_add(receipt.mana_output.b);
            pool.red = pool.red.saturating_add(receipt.mana_output.r);
            pool.green = pool.green.saturating_add(receipt.mana_output.g);
            pool.colorless = pool.colorless.saturating_add(receipt.mana_output.c);
        }
        self.record_committed_cost_events(
            payment.trigger_events.clone(),
            payment.sacrificed.clone(),
        );
        let cost_triggers = receipt.cost_triggers.clone();
        if receipt.simple_tap && cost_triggers.is_empty() && receipt.mana_damage.is_none() {
            self.state
                .undoable_mana_abilities
                .push(UndoableManaAbility {
                    player,
                    source: receipt.source.object_id,
                    produced: receipt.mana_output,
                    restriction_group_id,
                });
        }
        self.stage_triggers(cost_triggers);

        let mut events = payment.move_events.clone();
        if payment.life_paid > 0 {
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: player,
                    new_total: self.state.players[player_index].life,
                    delta: -(payment.life_paid as i32),
                })),
            });
        }
        if receipt.mana_damage.is_some() && !receipt.mana_damage_results.is_empty() {
            self.state.undoable_mana_abilities.clear();
            self.apply_attack_mana_damage_receipt(
                &receipt.mana_damage_results,
                &receipt.mana_damage_triggers,
                &mut events,
            )?;
        }
        if let (Some(old_group), Some(new_group)) =
            (receipt.restriction_group_id, restriction_group_id)
        {
            if old_group != new_group {
                restriction_group_remap.insert(old_group, new_group);
            }
        }
        Ok(ReappliedAttackManaReceipt {
            cost_plan,
            payment,
            restriction_group_id,
            mana_sources_consumed,
            events,
        })
    }

    fn apply_attack_mana_damage_receipt(
        &mut self,
        completed: &[super::damage::CompletedDamage],
        trigger_snapshot: &[super::triggers::CollectedTrigger],
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let mut depleted_effects = Vec::new();
        for damage in completed {
            for (effect_id, amount) in &damage.prevention_debits {
                let Some(effect) = self
                    .state
                    .damage_prevention_effects
                    .iter_mut()
                    .find(|effect| effect.id == *effect_id)
                else {
                    return Err(EngineError::Illegal(
                        "later mana activation depends on a missing prevention receipt",
                    ));
                };
                let crate::state::DamagePreventionAmount::Remaining(remaining) = &mut effect.amount
                else {
                    return Err(EngineError::Illegal(
                        "later mana activation depends on a changed prevention receipt",
                    ));
                };
                if *remaining < *amount {
                    return Err(EngineError::Illegal(
                        "later mana activation depends on consumed prevention capacity",
                    ));
                }
                *remaining -= *amount;
                if *remaining == 0 {
                    depleted_effects.push(*effect_id);
                }
            }
        }
        if !depleted_effects.is_empty() {
            self.state
                .damage_prevention_effects
                .retain(|effect| !depleted_effects.contains(&effect.id));
        }
        self.reapply_completed_damage_batch_with_triggers(completed, trigger_snapshot, events)
    }

    /// CR 506.4: removal is permanent for this combat, even if a later instruction restores
    /// Creature. Keep empty blocker groups: their attackers remain blocked (CR 509.1h).
    pub(super) fn remove_combat_participants(
        &mut self,
        ids: &[ObjectId],
        out: &mut Vec<rv1::RuledEvent>,
    ) {
        let Some(combat) = self.state.combat.as_mut() else {
            return;
        };
        let removed: BTreeSet<_> = ids
            .iter()
            .copied()
            .filter(|id| {
                combat.attacking.contains(id)
                    || combat
                        .blockers
                        .values()
                        .any(|blockers| blockers.contains(id))
            })
            .collect();
        if removed.is_empty() {
            return;
        }
        combat.attacking.retain(|id| !removed.contains(id));
        combat
            .attack_assignments
            .retain(|id, _| !removed.contains(id));
        combat.blockers.retain(|id, _| !removed.contains(id));
        let mut invalidated = removed.clone();
        for (attacker, blockers) in &mut combat.blockers {
            let prior = blockers.len();
            blockers.retain(|id| !removed.contains(id));
            if blockers.len() != prior {
                invalidated.insert(*attacker);
            }
        }
        combat
            .damage_assignments
            .retain(|id, _| !invalidated.contains(id));
        combat
            .trample_player_damage
            .retain(|id, _| !invalidated.contains(id));
        combat
            .first_strike_attackers
            .retain(|id| !removed.contains(id));
        combat.first_strike_blockers.retain(|id, blockers| {
            blockers.retain(|blocker| !removed.contains(blocker));
            !removed.contains(id)
        });
        let assignments: Vec<_> = combat
            .blockers
            .iter()
            .map(|(id, blockers)| {
                (
                    *id,
                    blockers.len(),
                    combat.damage_assignments.contains_key(id),
                )
            })
            .collect();
        let declared = combat.blockers_declared;
        let needed = declared
            && assignments.into_iter().any(|(id, count, assigned)| {
                !assigned && self.attacker_needs_explicit_damage_assignment(id, count)
            });
        self.state.combat.as_mut().unwrap().damage_assignment_needed = needed;
        out.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::RemovedFromCombat(
                rv1::CreaturesRemovedFromCombat {
                    object_ids: removed.into_iter().collect(),
                },
            )),
        });
    }

    /// Run at committed instruction/cohort boundaries, never while projecting an entry.
    pub(super) fn reconcile_combat_characteristics(&mut self, out: &mut Vec<rv1::RuledEvent>) {
        let Some(combat) = self.state.combat.as_ref() else {
            return;
        };
        let participants: BTreeSet<_> = combat
            .attacking
            .iter()
            .chain(combat.blockers.values().flatten())
            .copied()
            .collect();
        let removed: Vec<_> = participants
            .into_iter()
            .filter(|id| {
                self.state
                    .objects
                    .get(id)
                    .is_none_or(|object| object.zone != Zone::Battlefield)
                    || self
                        .characteristics(*id)
                        .is_none_or(|value| !value.is_creature())
                    || combat.attack_assignments.get(id).is_some_and(|assignment| {
                        assignment.attacker.zone_change_generation
                            != self
                                .state
                                .zone_change_generation
                                .get(id)
                                .copied()
                                .unwrap_or(0)
                    })
            })
            .collect();
        self.remove_combat_participants(&removed, out);
    }

    fn return_unblocked_attacker_assignment(
        &self,
        player: PlayerId,
        object: &rv1::CostObjectRef,
        step_allowed: impl FnOnce(TurnStep) -> bool,
    ) -> Option<CombatAttackAssignment> {
        if self.state.active_player_id() != player
            || self.state.priority_player_id() != player
            || !step_allowed(self.state.turn_step)
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

    /// CR 702.190a: the object returned for Sneak must still be an unblocked attacking creature
    /// controlled by the caster in that caster's declare-blockers step. The blocker map retains
    /// an attacker key after its last blocker leaves, so key absence is the authoritative
    /// "unblocked" test rather than an empty current blocker list.
    pub(super) fn sneak_return_assignment(
        &self,
        player: PlayerId,
        object: &rv1::CostObjectRef,
    ) -> Option<CombatAttackAssignment> {
        self.return_unblocked_attacker_assignment(player, object, |step| {
            step == TurnStep::DeclareBlockers
        })
    }

    /// CR 702.49a: Ninjutsu may return an unblocked attacker after blockers are declared and
    /// throughout the remainder of combat, including either combat-damage step.
    pub(super) fn ninjutsu_return_assignment(
        &self,
        player: PlayerId,
        object: &rv1::CostObjectRef,
    ) -> Option<CombatAttackAssignment> {
        self.return_unblocked_attacker_assignment(player, object, |step| {
            matches!(
                step,
                TurnStep::DeclareBlockers
                    | TurnStep::FirstStrikeDamage
                    | TurnStep::CombatDamage
                    | TurnStep::EndCombat
            )
        })
    }

    pub(super) fn ninjutsu_return_candidates(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .combat
            .as_ref()
            .map(|combat| combat.attacking.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|object_id| {
                self.ninjutsu_return_assignment(
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

    /// CR 702.49a / 702.190b / 506.3: a resolving Ninjutsu or Sneak permanent inherits the paid
    /// creature's recipient, but it was never declared as an attacker. Only the current entrant
    /// and captured recipient are revalidated; declaration restrictions are intentionally not
    /// applied.
    pub(super) fn add_returned_attacker(
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
        let limits = self.attack_limits();
        if !self
            .attack_defenders()
            .iter()
            .any(|(defender, _)| limits.allows_defender(*defender))
        {
            return Vec::new();
        }
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
        // Every eligible creature shares these exact defender edges. The requirement solver
        // relies on this complete relation; extend both authorities for recipient-specific rules.
        let limits = self.attack_limits();
        let defenders: Vec<_> = self
            .attack_defenders()
            .into_iter()
            .filter(|(defender, _)| limits.allows_defender(*defender))
            .collect();
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

    pub(super) fn blocking_player_ids(&self) -> Vec<PlayerId> {
        let Some(combat) = self.state.combat.as_ref() else {
            return Vec::new();
        };
        self.state
            .defending_player_ids()
            .into_iter()
            .filter(|player| {
                combat
                    .attack_assignments
                    .values()
                    .any(|assignment| assignment.defending_player == *player)
            })
            .collect()
    }

    /// Advance the blocker-declaration queue, recording empty declarations for seats with no
    /// legal pair. Only a defender who can actually block needs to make a choice.
    pub(super) fn next_blocking_player_needing_declaration(&mut self) -> Option<PlayerId> {
        for defender in self.blocking_player_ids() {
            if self
                .state
                .combat
                .as_ref()
                .is_some_and(|combat| combat.blockers_declared_by.contains(&defender))
            {
                continue;
            }
            if !self.blocking_options(defender).0.is_empty() {
                return Some(defender);
            }
            self.state
                .combat
                .as_mut()
                .unwrap()
                .blockers_declared_by
                .push(defender);
        }
        None
    }

    /// CR 508.1d: the active player's eligible must-attack requirement pool this combat —
    /// untapped, not summoning-sick (unless Haste), non-Defender creatures with `must_attack_if_able`,
    /// when a defending player exists to attack. Single source of truth shared by `set_attackers`
    /// enforcement and the client-facing `LegalActions` gate. Restrictions may prevent the whole
    /// pool from attacking; minimum_attack_requirement_count gives the achievable maximum.
    pub(super) fn attack_requirement_ids(&self) -> Vec<ObjectId> {
        // CR 508.1d: the active player need not pay an attack cost merely to comply with an
        // additional requirement. A costed-only edge therefore cannot make a creature required.
        let limits = self.attack_limits();
        let has_untaxed_edge = self.attack_defenders().into_iter().any(|(defender, _)| {
            limits.allows_defender(defender) && self.attack_tax_per_attacker(defender) == 0
        });
        if !has_untaxed_edge {
            return Vec::new();
        }
        let ap = self.state.active_player_id();
        let mut out = Vec::new();
        let eligible = self.eligible_attacker_ids(ap);
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
            if !eligible.contains(&oid) {
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
        if !self
            .attack_limits()
            .declaration_allowed(parsed_assignments.values())
        {
            return Err(EngineError::Illegal("attack declaration exceeds maximum"));
        }
        let requirements = self.attack_requirement_ids();
        let satisfied = list.iter().filter(|oid| requirements.contains(oid)).count();
        if satisfied < self.minimum_attack_requirement_count() {
            return Err(EngineError::Illegal(
                "attack declaration satisfies too few requirements",
            ));
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
        let mut tapping_attackers = Vec::new();
        for &oid in &list {
            // CR 702.20b — Vigilance: attacking doesn't cause this creature to tap.
            let has_vigilance =
                self.effective_has_keyword(oid, tricerules_cards::Keyword::Vigilance);
            if !has_vigilance {
                tapping_attackers.push(oid);
            }
        }
        let pre_attack_state = self.state.clone();
        let tap_events = self.tap_permanents(ap, &tapping_attackers);
        let attack_tax = self.attack_tax_total(&parsed_assignments)?;
        if attack_tax > 0 {
            return self.begin_attack_tax_payment(PendingAttackTaxPaymentStart {
                attacking_player: ap,
                assignments,
                parsed_assignments,
                tapping_attackers: &tapping_attackers,
                tap_events: &tap_events,
                pre_attack_state,
                generic_mana_cost: attack_tax,
            });
        }
        self.finalize_attack_declaration(ap, assignments, parsed_assignments, tap_events, None)
    }

    fn finalize_attack_declaration(
        &mut self,
        attacking_player: PlayerId,
        assignments: &[rv1::AttackAssignment],
        parsed_assignments: HashMap<ObjectId, CombatAttackAssignment>,
        mut tap_events: Vec<GameEvent>,
        buffered_tap_triggers: Option<Vec<super::triggers::CollectedTrigger>>,
    ) -> Result<RuledEventBatch, EngineError> {
        let committed_assignments = assignments
            .iter()
            .filter_map(|wire| {
                let assignment = *parsed_assignments.get(&wire.attacker_object_id)?;
                let object = self.state.objects.get(&wire.attacker_object_id)?;
                let current_generation = self
                    .state
                    .zone_change_generation
                    .get(&wire.attacker_object_id)
                    .copied()
                    .unwrap_or(0);
                (object.zone == Zone::Battlefield
                    && current_generation == assignment.attacker.zone_change_generation
                    && self.controller_of(wire.attacker_object_id) == Some(attacking_player))
                .then_some((*wire, assignment))
            })
            .collect::<Vec<_>>();
        let attacking_ids = committed_assignments
            .iter()
            .map(|(_, assignment)| assignment.attacker.object_id)
            .collect::<Vec<_>>();
        let committed_engine_assignments = committed_assignments
            .iter()
            .map(|(_, assignment)| (assignment.attacker.object_id, *assignment))
            .collect::<HashMap<_, _>>();
        let attacks = committed_assignments
            .iter()
            .map(|(_, assignment)| AttackEdgeSnapshot {
                attacker: assignment.attacker,
                defender: assignment.defender,
                defending_player: assignment.defending_player,
            })
            .collect::<Vec<_>>();
        if let Some(combat) = self.state.combat.as_mut() {
            combat.attacking = attacking_ids.clone();
            combat.attack_assignments = committed_engine_assignments;
            combat.blockers.clear();
            combat.damage_assignments.clear();
            combat.trample_player_damage.clear();
            combat.damage_assignment_needed = false;
            combat.assign_combat_damage_phase = false;
            combat.attackers_declared = true;
            combat.blockers_declared_by.clear();
            combat.blockers_declared = false;
            combat.first_strike_attackers.clear();
            combat.first_strike_blockers.clear();
            combat.first_strike_damage_done = false;
        } else {
            self.state.combat = Some(CombatState {
                attacking: attacking_ids.clone(),
                attack_assignments: committed_engine_assignments,
                blockers: HashMap::new(),
                damage_assignments: HashMap::new(),
                trample_player_damage: HashMap::new(),
                damage_assignment_needed: false,
                attackers_declared: true,
                blockers_declared_by: Vec::new(),
                blockers_declared: false,
                assign_combat_damage_phase: false,
                first_strike_attackers: Vec::new(),
                first_strike_blockers: HashMap::new(),
                first_strike_damage_done: false,
            });
        }
        // MTG timing: after attackers are declared, the game remains in declare-attackers
        // and the active player receives priority before moving to declare blockers. Mana remains
        // in the pool until that step actually ends (CR 106.4).
        self.state.turn_step = TurnStep::DeclareAttackers;
        if let Some(ai) = self.state.player_idx(attacking_player) {
            self.state.priority_idx = ai;
        }
        self.state.passes_since_stack_change = 0;
        let mut b = RuledEventBatch::default();
        b.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::AttackersDeclared(
                rv1::AttackersDeclared {
                    attacking_player_id: attacking_player,
                    assignments: committed_assignments
                        .iter()
                        .map(|(wire, _)| *wire)
                        .collect(),
                },
            )),
        });
        let atk_names: Vec<String> = attacking_ids
            .iter()
            .map(|&oid| object_display_name(&self.state, self.registry, oid))
            .collect();
        b.events.push(ev_log(format!(
            "P{} attacks with {}",
            attacking_player,
            atk_names.join(", ")
        )));
        let attack_event = GameEvent::AttackersDeclared {
            attacking_player,
            attacks,
        };
        if let Some(mut tap_triggers) = buffered_tap_triggers {
            self.reconcile_combat_characteristics(&mut b.events);
            self.refresh_enduring_story_designations();
            self.record_committed_events(&tap_events);
            self.record_committed_events(std::slice::from_ref(&attack_event));
            let attack_triggers = self.collect_event_triggers(std::slice::from_ref(&attack_event));
            tap_triggers.extend(self.reserve_trigger_uses(attack_triggers));
            self.stage_reserved_triggers(tap_triggers);
        } else {
            tap_events.push(attack_event);
            self.fire_triggers(&tap_events, &mut b.events);
        }
        b.events.push(ev_priority_changed(self));
        Ok(b)
    }

    pub(super) fn commit_attack_declaration(
        &mut self,
        player: PlayerId,
        command: &rv1::CommitAttackDeclaration,
    ) -> Result<RuledEventBatch, EngineError> {
        let pending =
            self.state
                .pending_attack_declaration
                .as_ref()
                .ok_or(EngineError::Illegal(
                    "no attack declaration is awaiting payment",
                ))?;
        if pending.attacking_player_id != player
            || pending.transaction_id != command.transaction_id
            || pending.revision != command.expected_revision
        {
            return Err(EngineError::Illegal(
                "wrong actor or stale attack declaration",
            ));
        }
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your attack payment"));
        }
        let internal = self
            .pending_attack_declaration_internal
            .as_ref()
            .ok_or(EngineError::Illegal("missing attack declaration snapshot"))?
            .clone();
        let prepared = self.prepare_attack_payment_costs(
            player,
            pending.generic_mana_cost,
            &command.restricted_mana,
        )?;
        let selection = command
            .payment
            .as_ref()
            .ok_or(EngineError::Illegal("missing attack tax payment"))?;
        if selection.expected_state_revision != self.state.command_index
            || selection.source.is_some()
            || !selection.convoke.is_empty()
            || !selection.waterbend.is_empty()
        {
            return Err(EngineError::Illegal("invalid attack tax payment selection"));
        }
        let plan = prepared.finish_explicit(&self.state, selection, 0)?;
        let _payment = self.commit_cost_transaction(plan)?;
        self.pending_attack_declaration_internal = None;
        let pending = self.state.pending_attack_declaration.take().unwrap();
        self.finalize_attack_declaration(
            player,
            &pending.assignments,
            internal.assignments,
            vec![],
            Some(internal.tap_triggers),
        )
    }

    pub(super) fn cancel_attack_declaration(
        &mut self,
        player: PlayerId,
        command: &rv1::CancelAttackDeclaration,
    ) -> Result<RuledEventBatch, EngineError> {
        let pending =
            self.state
                .pending_attack_declaration
                .as_ref()
                .ok_or(EngineError::Illegal(
                    "no attack declaration is awaiting payment",
                ))?;
        if pending.attacking_player_id != player
            || pending.transaction_id != command.transaction_id
            || pending.revision != command.expected_revision
            || self.state.priority_player_id() != player
        {
            return Err(EngineError::Illegal(
                "wrong actor or stale attack declaration",
            ));
        }
        let internal = self
            .pending_attack_declaration_internal
            .take()
            .ok_or(EngineError::Illegal("missing attack declaration snapshot"))?;
        self.state.pending_attack_declaration = None;
        self.rollback_pending_attack_declaration(&internal);
        let mut batch = RuledEventBatch::default();
        batch
            .events
            .push(ev_log(format!("P{player} cancels the attack declaration")));
        batch.events.push(ev_priority_changed(self));
        Ok(batch)
    }

    pub(super) fn abort_attack_declaration_for_departure(
        &mut self,
        departing_player: PlayerId,
        events: &mut Vec<RuledEvent>,
    ) {
        if self
            .state
            .pending_attack_declaration
            .as_ref()
            .is_none_or(|pending| pending.attacking_player_id != departing_player)
        {
            return;
        }
        let Some(internal) = self.pending_attack_declaration_internal.take() else {
            return;
        };
        self.state.pending_attack_declaration = None;
        self.rollback_pending_attack_declaration(&internal);
        events.push(ev_log(format!(
            "P{departing_player}'s attack declaration is abandoned when they leave the game"
        )));
    }

    fn rollback_pending_attack_declaration(&mut self, internal: &PendingAttackDeclarationInternal) {
        for attacker in &internal.tapped_attackers {
            let current_generation = self
                .state
                .zone_change_generation
                .get(&attacker.object_id)
                .copied()
                .unwrap_or(0);
            if current_generation == attacker.zone_change_generation
                && self
                    .state
                    .objects
                    .get(&attacker.object_id)
                    .is_some_and(|object| object.zone == Zone::Battlefield && object.tapped)
            {
                super::set_tapped(&mut self.state, attacker.object_id, false);
            }
        }
        self.state.triggered_once = internal.pre_attack_state.triggered_once.clone();
        self.state.trigger_uses_this_turn =
            internal.pre_attack_state.trigger_uses_this_turn.clone();
        self.state.pending_triggers = internal.pre_attack_state.pending_triggers.clone();
        self.state.staged_trigger_groups = internal.pre_attack_state.staged_trigger_groups.clone();
        self.state.captured_spell_copies = internal.pre_attack_state.captured_spell_copies.clone();
        self.state.pending_trigger_order = internal.pre_attack_state.pending_trigger_order.clone();
        for receipt in &internal.mana_ability_receipts {
            self.stage_triggers(receipt.cost_triggers.clone());
            self.stage_triggers(receipt.mana_damage_triggers.clone());
        }
    }

    pub(super) fn set_blockers(
        &mut self,
        defending_player: PlayerId,
        pairs: &[rv1::BlockPair],
    ) -> Result<RuledEventBatch, EngineError> {
        // A duel may auto-declare empty blocks when no legal pair exists. Retain the historical
        // acceptance of a matching explicit empty declaration from that defender.
        if self.state.players.len() == 2
            && pairs.is_empty()
            && self
                .state
                .combat
                .as_ref()
                .is_some_and(|c| c.blockers_declared && c.blockers.is_empty())
            && self.state.sole_defending_player_id() == Some(defending_player)
        {
            let mut batch = RuledEventBatch::default();
            fill_legal(&mut batch, self);
            return Ok(batch);
        }
        if self.state.priority_player_id() != defending_player
            || !self.blocking_player_ids().contains(&defending_player)
            || self.state.combat.as_ref().is_none_or(|c| {
                c.blockers_declared || c.blockers_declared_by.contains(&defending_player)
            })
        {
            return Err(EngineError::Illegal("not your block declaration"));
        }
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
        // CR 702.19: trample attackers with 1+ blockers also require explicit damage assignment
        // (to split damage between blockers and the defending player).
        let damage_assignment_needed = attacker_to_blockers.iter().any(|(atk_id, blks)| {
            self.attacker_needs_explicit_damage_assignment(*atk_id, blks.len())
        });
        if let Some(c) = self.state.combat.as_mut() {
            c.blockers.extend(attacker_to_blockers);
            c.damage_assignments.clear();
            c.trample_player_damage.clear();
            c.damage_assignment_needed |= damage_assignment_needed;
            c.assign_combat_damage_phase = false;
            c.blockers_declared_by.push(defending_player);
        }
        let next_defender = self.next_blocking_player_needing_declaration();
        if next_defender.is_none() {
            self.state.combat.as_mut().unwrap().blockers_declared = true;
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
        if next_defender.is_none() {
            self.finalize_block_declarations(&mut b.events)?;
        }
        self.clear_step_mana_pools();
        // MTG timing: blockers are declared in declare-blockers, then players get priority
        // before the game advances into combat-damage where damage is actually dealt.
        self.state.turn_step = TurnStep::DeclareBlockers;
        if let Some(i) = self
            .state
            .player_idx(next_defender.unwrap_or(self.state.active_player_id()))
        {
            self.state.priority_idx = i;
        }
        self.state.passes_since_stack_change = 0;
        b.events
            .push(ev_log(format!("P{} {}", defending_player, block_line)));
        b.events.push(ev_priority_changed(self));
        fill_legal(&mut b, self);
        Ok(b)
    }

    pub(super) fn finalize_block_declarations(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let combat = self
            .state
            .combat
            .as_ref()
            .ok_or(EngineError::Illegal("combat?"))?;
        let all_pairs: Vec<rv1::BlockPair> = combat
            .attacking
            .iter()
            .flat_map(|attacker_id| {
                combat
                    .blockers
                    .get(attacker_id)
                    .into_iter()
                    .flatten()
                    .map(|blocker_id| rv1::BlockPair {
                        attacker_id: *attacker_id,
                        blocker_id: *blocker_id,
                    })
            })
            .collect();
        let block_edges: Vec<BlockEdgeSnapshot> = all_pairs
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
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::BlockersDeclared(
                rv1::BlockersDeclared {
                    block_pairs: all_pairs,
                },
            )),
        });
        self.fire_triggers(
            &[GameEvent::BlockersDeclared { edges: block_edges }],
            events,
        );
        Ok(())
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
            // Command settlement performs losses, departures, SBAs and triggers before
            // the first-strike priority window; the next damage step remains a later command.
        } else {
            // Emit PhaseChanged before resolving damage so the C++ client clears its
            // stack-object set before any combat damage triggers are pushed (StackPushed).
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
            // The shared post-command boundary settles this simultaneous batch before priority.
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
        self.state.combat_damage_priority_pending = true;
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
                                let delta = super::life_numeric::loss_delta(result.dealt)?;
                                super::history::commit_life_change_checked(
                                    &mut self.state,
                                    index,
                                    delta,
                                )?;
                                let total = life_lost.entry(player).or_default();
                                *total = total
                                    .checked_add(delta)
                                    .ok_or(EngineError::LifeNumericRange("combat life loss sum"))?;
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
                        )?,
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
                        af.damage = af
                            .damage
                            .checked_add(dmg_to_att)
                            .ok_or(EngineError::LifeNumericRange("combat damage sum"))?;
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
                        bf.damage = bf
                            .damage
                            .checked_add(dmg_to_blk)
                            .ok_or(EngineError::LifeNumericRange("combat damage sum"))?;
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
                let mut total_blocker_damage: u32 = 0;
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
                    total_blocker_damage = total_blocker_damage
                        .checked_add(dealt)
                        .ok_or(EngineError::LifeNumericRange("combat damage sum"))?;
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
                    af.damage = af
                        .damage
                        .checked_add(total_blocker_damage)
                        .ok_or(EngineError::LifeNumericRange("combat damage sum"))?;
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
                            bf.damage = bf
                                .damage
                                .checked_add(dmg_to_blk)
                                .ok_or(EngineError::LifeNumericRange("combat damage sum"))?;
                            // CR 702.2b: any damage from attacker with deathtouch is lethal.
                            if att_has_deathtouch && dmg_to_blk > 0 {
                                bf.deathtouch_damage = true;
                            }
                        }
                        total_att_lifelink = total_att_lifelink
                            .checked_add(dmg_to_blk)
                            .ok_or(EngineError::LifeNumericRange("combat damage sum"))?;
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
                                        let delta = super::life_numeric::loss_delta(result.dealt)?;
                                        super::history::commit_life_change_checked(
                                            &mut self.state,
                                            index,
                                            delta,
                                        )?;
                                        let total = life_lost.entry(player).or_default();
                                        *total = total.checked_add(delta).ok_or(
                                            EngineError::LifeNumericRange("combat life loss sum"),
                                        )?;
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
                                )?,
                            };
                            if trample_after > 0 {
                                let mut dealt_event = damage_event;
                                dealt_event.amount = trample_after;
                                damage_dealt_events.push(dealt_event);
                            }
                            total_att_lifelink = total_att_lifelink
                                .checked_add(trample_after)
                                .ok_or(EngineError::LifeNumericRange("combat damage sum"))?;
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
                        delta: lost,
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
            )? {
                trigger_events.push(event);
            }
        }
        trigger_events.extend(damage_dealt_events.into_iter().map(|mut event| {
            event.source.zone_change_generation = self
                .state
                .zone_change_generation
                .get(&event.source.object_id)
                .copied();
            GameEvent::DamageDealt { event }
        }));
        self.fire_triggers(&trigger_events, events);
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
