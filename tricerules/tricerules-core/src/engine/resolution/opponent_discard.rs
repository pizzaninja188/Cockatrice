//! One closed Onyx opponent discard/payment round. Actual discard receipts never decide payment.
use super::*;
use crate::state::{DiscardCompletion, OpponentDiscardPayment, PendingOpponentDiscardRound};

pub(super) fn begin(cx: &mut EffectCx<'_>, amount: u32) -> Result<EffectOutcome, EngineError> {
    let mut round = PendingOpponentDiscardRound {
        stack_id: cx.top.id,
        leaf_index: cx.effect_index,
        controller: cx.controller,
        amount,
        occurrence: format!(
            "opponent-discard:{}:{}:{}:{}",
            cx.engine.state.command_index, cx.top.id, cx.top.source_zone_change, cx.effect_index
        ),
        outcomes: Vec::new(),
        public_reveals: Vec::new(),
    };
    let mut choices = Vec::new();
    for player in player_recipients(cx, PlayerRecipient::EachOpponent) {
        let Some(index) = cx.engine.state.player_idx(player) else {
            continue;
        };
        let candidates = cx.engine.state.players[index]
            .hand
            .iter()
            .copied()
            .filter(|id| cx.engine.state.is_card_object(*id))
            .map(|id| {
                (
                    id,
                    cx.engine
                        .state
                        .zone_change_generation
                        .get(&id)
                        .copied()
                        .unwrap_or(0),
                )
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            round
                .outcomes
                .push((player, OpponentDiscardPayment::Unpayable));
        } else {
            round
                .outcomes
                .push((player, OpponentDiscardPayment::Pending));
            choices.push(crate::state::PendingPlayerDiscardChoice {
                player,
                candidate_generations: candidates,
                required: 1,
                variable: false,
                optional_exact: true,
                alternative_filter: None,
                alternative_candidates: Vec::new(),
            });
        }
    }
    if choices.is_empty() {
        cx.engine
            .settle_opponent_discard_round(&round, cx.spell_label, cx.events)?;
        return Ok(EffectOutcome::Continue);
    }
    zones::park_player_set_discard_choice(
        cx.engine,
        cx.events,
        cx.parked_stack(),
        PendingPlayerSetDiscard {
            choices,
            current: 0,
            selections: Vec::new(),
            completion: DiscardCompletion::OpponentRound(round),
        },
    );
    Ok(EffectOutcome::Suspended)
}

impl GameEngine {
    pub(in crate::engine) fn begun_opponent_discard_round(
        &self,
        pending: &PendingResolution,
        replacement: Option<&crate::engine::replacement::PendingReplacementEvent>,
    ) -> bool {
        let (stack, completion) = match &pending.continuation {
            ResolutionContinuation::PlayerSetDiscard { stack, discard }
                if replacement.is_none() =>
            {
                (stack, &discard.completion)
            }
            ResolutionContinuation::DiscardReplacement { stack } => {
                let Some(crate::engine::replacement::PendingReplacementEvent::Discard(batch)) =
                    replacement
                else {
                    return false;
                };
                (stack, &batch.completion)
            }
            _ => return false,
        };
        let DiscardCompletion::OpponentRound(round) = completion else {
            return false;
        };
        pending.presentation.source_object_id == stack.item.id
            && Self::opponent_discard_frame_matches(stack, round)
    }

    pub(in crate::engine) fn opponent_discard_frame_matches(
        stack: &ParkedStackResolution,
        round: &PendingOpponentDiscardRound,
    ) -> bool {
        let effects = if let Some(ability) = &stack.item.activated_ability {
            if ability.program.is_some() || ability.modal.is_some() {
                return false;
            }
            &ability.effect
        } else if let Some(ability) = &stack.item.triggered_ability {
            if ability.program.is_some() || ability.modal.is_some() {
                return false;
            }
            &ability.effect
        } else {
            return false;
        };
        round.stack_id == stack.item.id
            && round.controller == stack.item.controller
            && stack.program().is_none()
            && stack.item.chosen_modes.is_empty()
            && stack.resume_effect_index() == round.leaf_index.checked_add(1)
            && matches!(effects.get(round.leaf_index as usize),
                Some(SpellEffectKind::EachOpponentMayDiscardOrLoseLife { amount }) if *amount == round.amount)
    }

    /// Validate the saved owner and every surviving hand incarnation before publishing any
    /// restored private offer. Departed rows are retired later; they never get a delegate.
    pub(in crate::engine) fn guard_saved_opponent_discard_round(
        &self,
        pending: &PendingResolution,
        replacement: Option<&crate::engine::replacement::PendingReplacementEvent>,
    ) -> Result<(), EngineError> {
        let choice_round = matches!(&pending.continuation,
            ResolutionContinuation::PlayerSetDiscard { discard, .. }
                if matches!(discard.completion, DiscardCompletion::OpponentRound(_)));
        let batch_round = matches!(replacement,
            Some(crate::engine::replacement::PendingReplacementEvent::Discard(batch))
                if matches!(batch.completion, DiscardCompletion::OpponentRound(_)));
        // The captured instruction authenticates the required completion; it never constructs
        // a replacement purpose or reconstructs lost round outcomes from an effect index.
        let captured_round = pending.continuation.stack().is_some_and(|stack| {
            let Some(leaf) = stack
                .resume_effect_index()
                .and_then(|next| next.checked_sub(1))
            else {
                return false;
            };
            let effects = stack
                .item
                .activated_ability
                .as_ref()
                .map(|ability| &ability.effect)
                .or_else(|| {
                    stack
                        .item
                        .triggered_ability
                        .as_ref()
                        .map(|ability| &ability.effect)
                });
            stack.program().is_none()
                && effects.is_some_and(|effects| {
                    matches!(
                        effects.get(leaf as usize),
                        Some(SpellEffectKind::EachOpponentMayDiscardOrLoseLife { .. })
                    )
                })
        });
        if !choice_round && !batch_round && !captured_round {
            return Ok(());
        }
        if !self.begun_opponent_discard_round(pending, replacement) {
            return Err(EngineError::ProgramInvariant(
                "opponent discard parent and completion do not match",
            ));
        }
        let current = match &pending.continuation {
            ResolutionContinuation::PlayerSetDiscard { discard, .. } => {
                self.opponent_discard_choices_are_current(discard)
            }
            ResolutionContinuation::DiscardReplacement { .. } => {
                let Some(crate::engine::replacement::PendingReplacementEvent::Discard(batch)) =
                    replacement
                else {
                    unreachable!();
                };
                let mut live_batch = (**batch).clone();
                live_batch.cards.retain(|card| {
                    self.state
                        .player_idx(card.player)
                        .is_some_and(|index| !self.state.players[index].has_lost)
                });
                self.discard_batch_current(&live_batch)
            }
            _ => false,
        };
        if !current {
            return Err(EngineError::ProgramInvariant(
                "saved opponent discard hand cohort is stale",
            ));
        }
        Ok(())
    }

    pub(in crate::engine) fn opponent_discard_choices_are_current(
        &self,
        discard: &PendingPlayerSetDiscard,
    ) -> bool {
        discard.current <= discard.choices.len()
            && discard.selections.len() == discard.current
            && discard.choices.iter().enumerate().all(|(index, choice)| {
                if self
                    .state
                    .player_idx(choice.player)
                    .is_none_or(|index| self.state.players[index].has_lost)
                {
                    return true;
                }
                let current = |id, generation| {
                    self.state.objects.get(&id).is_some_and(|object| {
                        object.owner == choice.player && object.zone == Zone::Hand
                    }) && self.state.is_card_object(id)
                        && self.state.players[self.state.player_idx(choice.player).unwrap()]
                            .hand
                            .contains(&id)
                        && self
                            .state
                            .zone_change_generation
                            .get(&id)
                            .copied()
                            .unwrap_or(0)
                            == generation
                };
                if index < discard.current {
                    let selected = &discard.selections[index];
                    choice.selection_cause(selected).is_some()
                        && selected.iter().all(|id| {
                            choice
                                .candidate_generations
                                .iter()
                                .find(|(candidate, _)| candidate == id)
                                .is_some_and(|&(id, generation)| current(id, generation))
                        })
                } else {
                    choice
                        .candidate_generations
                        .iter()
                        .all(|&(id, generation)| current(id, generation))
                }
            })
    }

    pub(in crate::engine) fn settle_opponent_discard_round(
        &mut self,
        round: &PendingOpponentDiscardRound,
        reason: &str,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        for &(player, outcome) in &round.outcomes {
            if self
                .state
                .player_idx(player)
                .is_none_or(|index| self.state.players[index].has_lost)
            {
                continue;
            }
            match outcome {
                OpponentDiscardPayment::Pending => {
                    return Err(EngineError::ProgramInvariant(
                        "opponent round has an unanswered live participant",
                    ))
                }
                OpponentDiscardPayment::LegallyChosenToPay => {}
                OpponentDiscardPayment::Declined | OpponentDiscardPayment::Unpayable => {
                    life::apply_life_loss(self, events, player, round.amount, reason)?;
                }
            }
        }
        Ok(())
    }

    /// Consume the selected round into the one discard-batch owner. The old choice owner is
    /// local and emptied here; observer/replacement suspension retains only the batch's copy.
    pub(in crate::engine) fn complete_opponent_discard_choices(
        &mut self,
        stack: ParkedStackResolution,
        mut discard: PendingPlayerSetDiscard,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let DiscardCompletion::OpponentRound(mut round) = std::mem::take(&mut discard.completion)
        else {
            return Err(EngineError::ProgramInvariant(
                "opponent round completion missing",
            ));
        };
        if !Self::opponent_discard_frame_matches(&stack, &round) {
            return Err(EngineError::ProgramInvariant(
                "opponent round parent frame changed",
            ));
        }
        let selected = discard
            .choices
            .iter()
            .zip(&discard.selections)
            .flat_map(|(choice, selection)| {
                selection
                    .iter()
                    .map(|&id| (choice.player, id, crate::state::DiscardCause::Effect))
            })
            .collect::<Vec<_>>();
        let ids = selected.iter().map(|(_, id, _)| *id).collect::<Vec<_>>();
        let label = self
            .registry
            .get(&stack.item.card_id)
            .and_then(|definition| definition.face(stack.item.face_index))
            .map(|face| face.name.clone())
            .unwrap_or_else(|| stack.item.card_id.clone());
        let mut reveals = crate::engine::reveals::reveal_cards(
            &self.state,
            self.registry,
            &ids,
            stack.item.id,
            &label,
        );
        for (index, event) in reveals.iter_mut().enumerate() {
            if let Some(rv1::ruled_event::Ev::CardsRevealed(reveal)) = event.ev.as_mut() {
                reveal.reveal_id = format!(
                    "{}:{}:{index}",
                    round.occurrence, reveal.zone_owner_player_id
                );
                round.public_reveals.push(reveal.clone());
            }
        }
        events.extend(reveals);
        let mut completed = self.start_discard_replacements_with_completion(
            stack,
            selected,
            true,
            DiscardCompletion::OpponentRound(round),
        )?;
        events.extend(completed.events);
        completed.events = events;
        Ok(completed)
    }
}
