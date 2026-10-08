//! A replacement owns its action until every library choice completes (CR 121.6 / 614.6).
use super::*;

impl GameEngine {
    pub(super) fn draw_library_choice(
        &self,
        work: &PendingDrawTransaction,
    ) -> rv1::ResolutionChoiceRequired {
        let action = work.action.as_ref().expect("draw library action");
        let mut choice = rv1::ResolutionChoiceRequired {
            deciding_player_id: match &action.stage {
                DrawLibraryStage::ZurPayment { payers, cursor, .. } => payers
                    .get(*cursor)
                    .copied()
                    .unwrap_or_else(|| work.requests.front().expect("drawer").player),
                _ => work.requests.front().expect("drawer").player,
            },
            source_object_id: action.source,
            min: 1,
            max: 1,
            ..Default::default()
        };
        match &action.stage {
            DrawLibraryStage::Optional | DrawLibraryStage::ChooseKind => {
                let (prompt, labels) = if matches!(action.stage, DrawLibraryStage::Optional) {
                    (
                        "Choose whether to replace this draw.",
                        ["Replace this draw", "Draw normally"],
                    )
                } else {
                    ("Choose land or nonland.", ["Land", "Nonland"])
                };
                choice.prompt_text = prompt.into();
                choice.choice_kind = rv1::ChoiceKind::ResolutionBranch as i32;
                choice.resolution_branches = labels
                    .into_iter()
                    .enumerate()
                    .map(|(index, label)| rv1::ResolutionBranchOption {
                        branch_index: index as u32,
                        label: label.into(),
                        selectable: true,
                        ..Default::default()
                    })
                    .collect();
            }
            DrawLibraryStage::ChooseToHand(cards) | DrawLibraryStage::OrderBottom(cards) => {
                let ordered = matches!(action.stage, DrawLibraryStage::OrderBottom(_));
                choice.choice_kind = rv1::ChoiceKind::LibraryLook as i32;
                choice.prompt_text = if ordered {
                    "Order these cards on the bottom of your library."
                } else {
                    "Look at these cards. Choose one to put into your hand."
                }
                .into();
                choice.ordered = ordered;
                if ordered {
                    choice.min = cards.len() as u32;
                    choice.max = choice.min;
                }
                choice.candidate_object_ids = cards.iter().map(|(oid, _)| *oid).collect();
                choice.candidate_card_ids = cards
                    .iter()
                    .map(|(oid, _)| self.state.objects[oid].card_id.clone())
                    .collect();
                choice.candidate_names = cards
                    .iter()
                    .map(|(oid, _)| {
                        super::super::events::object_display_name(&self.state, self.registry, *oid)
                    })
                    .collect();
                choice.candidate_selectable = vec![true; cards.len()];
                // The full reveal (including the matched card) is a separate occurrence.
                // Bottom candidates/order stay private; attaching that full snapshot here would
                // violate the client's requirement that reveal cards equal choice candidates.
            }
            DrawLibraryStage::ZurPayment { .. } => {
                choice.choice_kind = rv1::ChoiceKind::ResolutionBranch as i32;
                choice.prompt_text =
                    "Pay 2 life to put the revealed card into its owner's graveyard?".into();
                let can_pay = self
                    .state
                    .player_idx(choice.deciding_player_id)
                    .is_some_and(|index| {
                        !self.state.players[index].has_lost && self.state.players[index].life >= 2
                    });
                choice.resolution_branches = vec![
                    rv1::ResolutionBranchOption {
                        branch_index: 0,
                        label: "Pay 2 life".into(),
                        selectable: can_pay,
                        ..Default::default()
                    },
                    rv1::ResolutionBranchOption {
                        branch_index: 1,
                        label: "Decline".into(),
                        selectable: true,
                        ..Default::default()
                    },
                ];
            }
        }
        choice
    }

    pub(super) fn park_draw_library_action(
        &mut self,
        work: PendingDrawTransaction,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> DrawProgress {
        let choice = self.draw_library_choice(&work);
        let stack = work.completion.stack().cloned();
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player: choice.deciding_player_id,
            presentation: PendingResolutionPresentation {
                source_object_id: choice.source_object_id,
                candidates: choice.candidate_object_ids.clone(),
                min: choice.min,
                max: choice.max,
                ordered: choice.ordered,
                unique_names: false,
                prompt: choice.prompt_text.clone(),
                choice_kind: rv1::ChoiceKind::try_from(choice.choice_kind)
                    .expect("draw choice kind"),
            },
            continuation: ResolutionContinuation::DrawReplacement { stack },
        });
        self.state.pending_replacement_event = Some(PendingReplacementEvent::Draw(Box::new(work)));
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)),
        });
        DrawProgress::Parked
    }

    pub(in crate::engine) fn draw_action_reveal(&self) -> Option<rv1::CardsRevealed> {
        let Some(PendingReplacementEvent::Draw(work)) = &self.state.pending_replacement_event
        else {
            return None;
        };
        work.action.as_ref()?.reveal.clone()
    }

    pub(super) fn finish_draw_library_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let Some(PendingReplacementEvent::Draw(work)) = &self.state.pending_replacement_event
        else {
            unreachable!()
        };
        let action = work.action.as_ref().expect("draw action");
        let drawer = work.requests.front().expect("drawer").player;
        let player = match &action.stage {
            DrawLibraryStage::ZurPayment { payers, cursor, .. } => {
                payers.get(*cursor).copied().unwrap_or(drawer)
            }
            _ => drawer,
        };
        let expected_decision = match &action.stage {
            DrawLibraryStage::Optional
            | DrawLibraryStage::ChooseKind
            | DrawLibraryStage::ZurPayment { .. } => rv1::ResolutionChoiceDecision::SelectBranch,
            DrawLibraryStage::ChooseToHand(_) | DrawLibraryStage::OrderBottom(_) => {
                rv1::ResolutionChoiceDecision::Unspecified
            }
        };
        let clean = decision == expected_decision
            && answer.chosen_player_ids.is_empty()
            && answer.cast_spell.is_none()
            && answer.chosen_combat_defender.is_none()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty()
            && answer.spell_cast_announcement.is_none();
        let chosen = &answer.chosen_object_ids;
        let valid = clean
            && match &action.stage {
                DrawLibraryStage::Optional | DrawLibraryStage::ChooseKind => {
                    chosen.is_empty() && answer.selected_branch_index < 2
                }
                DrawLibraryStage::ZurPayment { .. } => {
                    chosen.is_empty()
                        && answer.selected_branch_index < 2
                        && (answer.selected_branch_index != 0
                            || self.state.player_idx(player).is_some_and(|index| {
                                !self.state.players[index].has_lost
                                    && self.state.players[index].life >= 2
                            }))
                }
                DrawLibraryStage::ChooseToHand(cards) | DrawLibraryStage::OrderBottom(cards) => {
                    let count = if matches!(action.stage, DrawLibraryStage::ChooseToHand(_)) {
                        1
                    } else {
                        cards.len()
                    };
                    answer.selected_branch_index == 0
                        && chosen.len() == count
                        && chosen.iter().collect::<BTreeSet<_>>().len() == chosen.len()
                        && chosen
                            .iter()
                            .all(|oid| cards.iter().any(|(candidate, _)| candidate == oid))
                        && super::super::library_choices::current(self, player, cards)
                        && self.state.player_idx(player).is_some_and(|index| {
                            cards
                                .iter()
                                .map(|(oid, _)| *oid)
                                .eq(self.state.players[index]
                                    .library
                                    .iter()
                                    .take(cards.len())
                                    .copied())
                        })
                }
            };
        if !valid {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "invalid or stale draw replacement action choice",
            ));
        }
        let Some(PendingReplacementEvent::Draw(work)) = self.state.pending_replacement_event.take()
        else {
            unreachable!()
        };
        let mut work = *work;
        let ResolutionContinuation::DrawReplacement { stack } = pending.continuation else {
            unreachable!()
        };
        work.completion.transfer_stack(stack);
        let mut action = work.action.take().expect("draw action");
        let mut events = Vec::new();
        match action.stage {
            DrawLibraryStage::Optional => {
                if answer.selected_branch_index == 0 {
                    action.stage = DrawLibraryStage::ChooseKind;
                    work.action = Some(action);
                }
            }
            DrawLibraryStage::ChooseKind => {
                // Applying the action consumes this draw even when no matching card exists.
                work.nodes.pop().expect("replaced draw node");
                let index = self.state.player_idx(player).expect("drawer");
                let land = answer.selected_branch_index == 0;
                let mut revealed = Vec::new();
                let mut found = None;
                for oid in &self.state.players[index].library {
                    revealed.push(*oid);
                    if self
                        .registry
                        .get(&self.state.objects[oid].card_id)
                        .is_some_and(|card| card.primary_face().is_land == land)
                    {
                        found = Some(*oid);
                        break;
                    }
                }
                action.reveal = super::super::reveals::reveal_choice(
                    &self.state,
                    self.registry,
                    &revealed,
                    action.source,
                    &action.source_label,
                );
                if let Some(reveal) = action.reveal.as_mut() {
                    reveal.reveal_id = format!(
                        "draw:{}:{}:{}",
                        self.state.command_index, action.source, player
                    );
                    events.push(rv1::RuledEvent {
                        ev: Some(rv1::ruled_event::Ev::CardsRevealed(reveal.clone())),
                    });
                }
                let chosen: Vec<_> = found.into_iter().collect();
                super::super::library_choices::put_into_hand(
                    self,
                    player,
                    &chosen,
                    true,
                    &mut events,
                )?;
                revealed.retain(|oid| !chosen.contains(oid));
                self.finish_draw_library_bottom(&mut work, action, revealed, &mut events);
            }
            DrawLibraryStage::ChooseToHand(ref cards) => {
                let remaining = cards
                    .iter()
                    .map(|(oid, _)| *oid)
                    .filter(|oid| !chosen.contains(oid))
                    .collect();
                super::super::library_choices::put_into_hand(
                    self,
                    player,
                    chosen,
                    false,
                    &mut events,
                )?;
                self.finish_draw_library_bottom(&mut work, action, remaining, &mut events);
            }
            DrawLibraryStage::OrderBottom(_) => {
                self.order_draw_library_bottom(player, chosen, &mut events);
            }
            DrawLibraryStage::ZurPayment {
                payers,
                mut cursor,
                mut pay_intents,
            } => {
                if answer.selected_branch_index == 0 {
                    pay_intents.push(player);
                    events.push(super::super::events::ev_log(format!(
                        "P{player} chooses to pay 2 life for Zur's Weirding."
                    )));
                } else {
                    events.push(super::super::events::ev_log(format!(
                        "P{player} declines to pay 2 life for Zur's Weirding."
                    )));
                }
                cursor += 1;
                action.stage = DrawLibraryStage::ZurPayment {
                    payers,
                    cursor,
                    pay_intents,
                };
                work.action = Some(action);
            }
        }
        match self.advance_draw_transaction(work, &mut events)? {
            DrawProgress::Parked | DrawProgress::GameEnded => Ok(finish_with_events(self, events)),
            DrawProgress::Complete(done) => self.complete_draw_transaction(done, events),
        }
    }

    fn finish_draw_library_bottom(
        &mut self,
        work: &mut PendingDrawTransaction,
        mut action: DrawLibraryAction,
        remaining: Vec<ObjectId>,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        if remaining.len() > 1 {
            action.stage = DrawLibraryStage::OrderBottom(super::super::library_choices::capture(
                self, &remaining,
            ));
            work.action = Some(action);
        } else {
            self.order_draw_library_bottom(
                work.requests.front().expect("drawer").player,
                &remaining,
                events,
            );
        }
    }

    fn order_draw_library_bottom(
        &mut self,
        player: PlayerId,
        cards: &[ObjectId],
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let index = self.state.player_idx(player).expect("drawer");
        self.state.players[index]
            .library
            .retain(|oid| !cards.contains(oid));
        self.state.players[index]
            .library
            .extend(cards.iter().copied());
        events.push(ev_log(format!(
            "P{player} puts {} cards on the bottom of their library.",
            cards.len()
        )));
        events.push(super::super::events::ev_log_private(
            format!(
                "P{player} orders {}.",
                cards
                    .iter()
                    .map(|oid| super::super::events::object_display_name(
                        &self.state,
                        self.registry,
                        *oid
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            player,
        ));
    }
}
