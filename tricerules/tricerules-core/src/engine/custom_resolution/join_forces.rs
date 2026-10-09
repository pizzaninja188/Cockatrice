use super::*;

impl GameEngine {
    pub(super) fn finish_join_forces_payment(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
        player: PlayerId,
    ) -> Result<RuledEventBatch, EngineError> {
        let (stack, payment, payer_order, next_payer, total_paid) = match &pending.continuation {
            ResolutionContinuation::JoinForces {
                stack,
                payment,
                payer_order,
                next_payer,
                total_paid,
            } => (
                stack.clone(),
                payment.clone(),
                payer_order.clone(),
                *next_payer,
                *total_paid,
            ),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("not a Join Forces contribution"));
            }
        };

        if decision != rv1::ResolutionChoiceDecision::PayMana
            || !answer.chosen_object_ids.is_empty()
            || !answer.chosen_player_ids.is_empty()
            || answer.selected_branch_index != 0
            || answer.cast_spell.is_some()
            || answer.spell_cast_announcement.is_some()
            || answer.chosen_combat_defender.is_some()
            || payer_order.get(next_payer) != Some(&player)
            || self
                .state
                .player_idx(player)
                .is_none_or(|index| self.state.players[index].has_lost)
        {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("invalid Join Forces contribution"));
        }

        let payment_result = (|| {
            let mut costs =
                self.prepare_resolution_payment_costs(player, &payment, &answer.restricted_mana)?;
            let selection = answer.payment.as_ref().ok_or(EngineError::Illegal(
                "Join Forces payment was not previewed",
            ))?;
            if !selection.convoke.is_empty() || !selection.waterbend.is_empty() {
                return Err(EngineError::Illegal(
                    "Join Forces accepts mana contributions only",
                ));
            }
            let amount = super::super::payment::convoke::selected_mana_total(
                selection.mana.as_ref(),
                &answer.restricted_mana,
            )?;
            costs.mana = ManaCost {
                pips: vec![tricerules_card_model::mana::ManaSymbol::Generic(amount)],
            };
            let life = self.validate_explicit_payment(
                player,
                pending.presentation.source_object_id,
                false,
                &costs,
                selection,
            )?;
            let plan = costs.finish_explicit(&self.state, selection, life)?;
            let amount = plan.mana_spent().ok_or(EngineError::Illegal(
                "Join Forces payment has no mana debit",
            ))?;
            let amount = u32::try_from(amount)
                .map_err(|_| EngineError::Illegal("Join Forces contribution overflow"))?;
            let total = total_paid
                .checked_add(amount)
                .ok_or(EngineError::Illegal("Join Forces total overflow"))?;
            let receipt = self.commit_cost_transaction(plan)?;
            Ok((amount, total, receipt))
        })();
        let (amount, total_paid, receipt) = match payment_result {
            Ok(result) => result,
            Err(error) => {
                self.state.pending_resolution = Some(pending);
                return Err(error);
            }
        };

        let mut events = Vec::new();
        self.fire_resolution_cost_triggers(receipt.trigger_events, receipt.sacrificed, &mut events);
        events.extend(receipt.move_events);
        let spell_label = self
            .registry
            .get(&stack.item.card_id)
            .map(|definition| definition.name.as_str())
            .unwrap_or(&stack.item.card_id);
        events.push(ev_log(format!(
            "P{player} contributes {amount} mana to {spell_label}; the total is {total_paid}."
        )));
        // The accepted choice is consequential even for zero mana. Mana abilities used while
        // resolving are committed, and earlier floating mana remains available to later steps.
        self.state.undoable_mana_abilities.clear();

        let mut next = next_payer + 1;
        while let Some(candidate) = payer_order.get(next) {
            if self
                .state
                .player_idx(*candidate)
                .is_some_and(|index| !self.state.players[index].has_lost)
            {
                break;
            }
            next += 1;
        }
        if let Some(&next_player) = payer_order.get(next) {
            let mut next_payment = payment;
            next_payment.undo_history_start = self.state.undoable_mana_abilities.len();
            self.state.pending_resolution = Some(PendingResolution {
                deciding_player: next_player,
                presentation: pending.presentation,
                continuation: ResolutionContinuation::JoinForces {
                    stack,
                    payment: next_payment,
                    payer_order,
                    next_payer: next,
                    total_paid,
                },
            });
            events.push(
                self.resolution_payment_choice_event()
                    .ok_or(EngineError::Illegal(
                        "Join Forces continuation prompt missing",
                    ))?,
            );
            return Ok(finish_with_events(self, events));
        }

        let mut previous_result = stack.previous_result;
        previous_result.mana_paid = Some(total_paid);
        self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            previous_result,
            events,
        )
    }

    /// A departing payer's current Join Forces choice is not delegated. Advance to the next
    /// surviving player, or resume with the contributions already committed.
    pub(in crate::engine) fn advance_departed_join_forces_payer(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(());
        };
        let ResolutionContinuation::JoinForces {
            payer_order,
            next_payer,
            ..
        } = &pending.continuation
        else {
            return Ok(());
        };
        let current_departed = payer_order.get(*next_payer).is_none_or(|payer| {
            self.state
                .player_idx(*payer)
                .is_none_or(|index| self.state.players[index].has_lost)
        });
        if !current_departed {
            return Ok(());
        }

        let mut pending = self
            .state
            .pending_resolution
            .take()
            .expect("Join Forces pending resolution was observed");
        let ResolutionContinuation::JoinForces {
            stack,
            mut payment,
            payer_order,
            next_payer,
            total_paid,
        } = pending.continuation
        else {
            unreachable!("continuation checked above")
        };
        let next = (next_payer..payer_order.len()).find(|&index| {
            self.state
                .player_idx(payer_order[index])
                .is_some_and(|player_index| !self.state.players[player_index].has_lost)
        });
        if let Some(next) = next {
            payment.undo_history_start = self.state.undoable_mana_abilities.len();
            pending.deciding_player = payer_order[next];
            pending.continuation = ResolutionContinuation::JoinForces {
                stack,
                payment,
                payer_order,
                next_payer: next,
                total_paid,
            };
            self.state.pending_resolution = Some(pending);
            events.push(
                self.resolution_payment_choice_event()
                    .ok_or(EngineError::Illegal(
                        "Join Forces continuation prompt missing",
                    ))?,
            );
            return Ok(());
        }

        let mut previous_result = stack.previous_result;
        previous_result.mana_paid = Some(total_paid);
        let completed = self.complete_parked_resolution_with_previous(
            stack.item,
            stack.resume_effect_index,
            previous_result,
            std::mem::take(events),
        )?;
        *events = completed.events;
        Ok(())
    }
}
