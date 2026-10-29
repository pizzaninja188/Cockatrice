//! Park and resume Kaya's finite direct `EachOpponentMayScry(1)` effect.
use super::*;
use crate::state::{
    PendingEachOpponentScryChoice as ScryChoice, PendingEachOpponentScryStage as ScryStage,
    PendingResolutionPresentation, ResolutionPromptId,
};

impl GameEngine {
    pub(in crate::engine) fn each_opponent_scry_choice_event(
        &self,
    ) -> Result<Option<rv1::RuledEvent>, EngineError> {
        let Some(pending) = self.state.pending_resolution.as_ref() else {
            return Ok(None);
        };
        self.each_opponent_scry_choice_event_for(pending)
    }

    fn each_opponent_scry_choice_event_for(
        &self,
        pending: &PendingResolution,
    ) -> Result<Option<rv1::RuledEvent>, EngineError> {
        let ResolutionContinuation::EachOpponentMayScry { scry, .. } = &pending.continuation else {
            return Ok(None);
        };
        let mut choice = rv1::ResolutionChoiceRequired {
            resolution_prompt_id: pending.resolution_prompt_id.get(),
            deciding_player_id: pending.deciding_player,
            source_object_id: pending.presentation.source_object_id,
            prompt_text: pending.presentation.prompt.clone(),
            choice_kind: pending.presentation.choice_kind as i32,
            candidate_object_ids: pending.presentation.candidates.clone(),
            min: pending.presentation.min,
            max: pending.presentation.max,
            ordered: pending.presentation.ordered,
            unique_names: pending.presentation.unique_names,
            ..Default::default()
        };
        match &scry.stage {
            ScryStage::OptIn { subject_index, .. } => {
                let subject =
                    *scry
                        .subjects
                        .get(*subject_index)
                        .ok_or(EngineError::ProgramInvariant(
                            "Kaya Scry1 subject index is invalid",
                        ))?;
                choice.resolution_branches = vec![
                    scry_branch_option(0, format!("P{subject} scries 1")),
                    scry_branch_option(1, "Decline".into()),
                ];
            }
            ScryStage::Placement {
                card_index,
                delegated_chooser,
            } => {
                let card = scry
                    .cards
                    .get(*card_index)
                    .ok_or(EngineError::ProgramInvariant(
                        "Kaya Scry1 card index is invalid",
                    ))?;
                if delegated_chooser.is_some() {
                    choice.resolution_branches = vec![
                        scry_branch_option(0, format!("Keep P{}'s card on top", card.subject)),
                        scry_branch_option(
                            1,
                            format!("Put P{}'s card on the bottom", card.subject),
                        ),
                    ];
                } else {
                    let (card_ids, names) =
                        super::resolution::candidate_identities(self, &[card.object_id]);
                    choice.candidate_card_ids = card_ids;
                    choice.candidate_names = names;
                    choice.candidate_selectable = vec![true];
                }
            }
            ScryStage::ChoosingDelegate {
                choice: for_choice,
                candidates,
            } => {
                let subject = scry_choice_subject(scry, *for_choice).ok_or(
                    EngineError::ProgramInvariant("Kaya Scry1 delegate subject is missing"),
                )?;
                choice.resolution_branches = super::resolution::resolution_choice_delegate_options(
                    &self.state,
                    scry.controller,
                    subject,
                    candidates,
                );
            }
        }
        Ok(Some(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)),
        }))
    }

    pub(super) fn finish_each_opponent_scry_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let rollback = pending.clone();
        let result = self.finish_each_opponent_scry_choice_inner(pending, answer, decision);
        if result.is_err() {
            self.state.pending_resolution = Some(rollback);
        }
        result
    }

    fn finish_each_opponent_scry_choice_inner(
        &mut self,
        mut pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let stage = match &pending.continuation {
            ResolutionContinuation::EachOpponentMayScry { scry, .. } => scry.stage.clone(),
            _ => {
                return self.reject_each_opponent_scry_choice(
                    pending,
                    "Kaya Scry1 continuation is missing",
                )
            }
        };
        let mut events = Vec::new();
        match stage {
            ScryStage::ChoosingDelegate { choice, candidates } => {
                if decision != rv1::ResolutionChoiceDecision::SelectBranch
                    || !clean_scry_branch_answer(answer)
                {
                    return self.reject_each_opponent_scry_choice(
                        pending,
                        "Kaya Scry1 delegate choice has unrelated or invalid data",
                    );
                }
                let ResolutionContinuation::EachOpponentMayScry { scry, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("continuation was checked above")
                };
                let subject = scry_choice_subject(scry, choice).ok_or(
                    EngineError::ProgramInvariant("Kaya Scry1 delegate subject is missing"),
                )?;
                let Some(delegate) = super::resolution::resolution_choice_delegate_at_index(
                    &self.state,
                    scry.controller,
                    subject,
                    &candidates,
                    answer.selected_branch_index,
                ) else {
                    return self.reject_each_opponent_scry_choice(
                        pending,
                        "Kaya Scry1 delegate is no longer eligible",
                    );
                };
                let selector = pending.deciding_player;
                scry.stage = match choice {
                    ScryChoice::OptIn { subject_index } => ScryStage::OptIn {
                        subject_index,
                        delegated_chooser: Some(delegate),
                    },
                    ScryChoice::Placement { card_index } => ScryStage::Placement {
                        card_index,
                        delegated_chooser: Some(delegate),
                    },
                };
                self.configure_each_opponent_scry_prompt(&mut pending)?;
                events.push(ev_log(format!(
                    "P{selector} chooses P{delegate} to make P{subject}'s scry choice."
                )));
                self.append_each_opponent_scry_prompt(&pending, &mut events)?;
                self.state.pending_resolution = Some(pending);
                Ok(finish_with_events(self, events))
            }
            ScryStage::OptIn {
                subject_index,
                delegated_chooser,
            } => {
                if decision != rv1::ResolutionChoiceDecision::SelectBranch
                    || answer.selected_branch_index > 1
                    || !clean_scry_branch_answer(answer)
                {
                    return self.reject_each_opponent_scry_choice(
                        pending,
                        "Kaya Scry1 opt-in requires a valid branch",
                    );
                }
                let ResolutionContinuation::EachOpponentMayScry { scry, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("continuation was checked above")
                };
                let Some(subject) = scry.subjects.get(subject_index).copied() else {
                    return self.reject_each_opponent_scry_choice(
                        pending,
                        "Kaya Scry1 subject index is invalid",
                    );
                };
                let chooser = delegated_chooser.unwrap_or(subject);
                if pending.deciding_player != chooser
                    || (delegated_chooser.is_none() && !self.scry_player_is_live(subject))
                    || (delegated_chooser.is_some() && self.scry_player_is_live(subject))
                    || (delegated_chooser.is_some() && !self.scry_player_is_live(chooser))
                {
                    return self.reject_each_opponent_scry_choice(
                        pending,
                        "Kaya Scry1 opt-in actor is stale",
                    );
                }
                if answer.selected_branch_index == 0 && self.scry_player_is_live(subject) {
                    scry.accepted_subjects.push(subject);
                }
                events.push(ev_log(if answer.selected_branch_index == 0 {
                    if delegated_chooser.is_some() {
                        format!("P{chooser} accepts P{subject}'s Scry 1.")
                    } else {
                        format!("P{subject} chooses to scry 1.")
                    }
                } else if delegated_chooser.is_some() {
                    format!("P{chooser} declines P{subject}'s Scry 1.")
                } else {
                    format!("P{subject} declines to scry.")
                }));
                self.advance_each_opponent_scry_opt_ins(pending, subject_index + 1, events)
            }
            ScryStage::Placement {
                card_index,
                delegated_chooser,
            } => {
                let ResolutionContinuation::EachOpponentMayScry { scry, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("continuation was checked above")
                };
                let Some(card) = scry.cards.get(card_index).cloned() else {
                    return self.reject_each_opponent_scry_choice(
                        pending,
                        "Kaya Scry1 card index is invalid",
                    );
                };
                let chooser = delegated_chooser.unwrap_or(card.subject);
                if pending.deciding_player != chooser
                    || (delegated_chooser.is_none() && !self.scry_player_is_live(card.subject))
                    || (delegated_chooser.is_some() && self.scry_player_is_live(card.subject))
                    || (delegated_chooser.is_some() && !self.scry_player_is_live(chooser))
                {
                    return self.reject_each_opponent_scry_choice(
                        pending,
                        "Kaya Scry1 placement actor is stale",
                    );
                }
                let put_bottom = if delegated_chooser.is_some() {
                    if decision != rv1::ResolutionChoiceDecision::SelectBranch
                        || answer.selected_branch_index > 1
                        || !clean_scry_branch_answer(answer)
                    {
                        return self.reject_each_opponent_scry_choice(
                            pending,
                            "delegated Kaya Scry1 placement requires a valid branch",
                        );
                    }
                    answer.selected_branch_index == 1
                } else {
                    if decision != rv1::ResolutionChoiceDecision::Unspecified
                        || answer.selected_branch_index != 0
                        || !clean_scry_object_answer(answer)
                        || answer
                            .chosen_object_ids
                            .iter()
                            .any(|oid| *oid != card.object_id)
                        || answer.chosen_object_ids.len() > 1
                    {
                        return self.reject_each_opponent_scry_choice(
                            pending,
                            "Kaya Scry1 placement must choose zero or one offered card",
                        );
                    }
                    if !self.current_scry_top_card(&card) {
                        return self.reject_each_opponent_scry_choice(
                            pending,
                            "stale Kaya Scry1 library cohort",
                        );
                    }
                    !answer.chosen_object_ids.is_empty()
                };
                let ResolutionContinuation::EachOpponentMayScry { scry, .. } =
                    &mut pending.continuation
                else {
                    unreachable!("continuation was checked above")
                };
                scry.cards[card_index].put_bottom = Some(put_bottom);
                events.push(ev_log(if put_bottom {
                    format!(
                        "P{chooser} chooses to put P{}'s looked-at card on the bottom.",
                        card.subject
                    )
                } else {
                    format!(
                        "P{chooser} chooses to keep P{}'s looked-at card on top.",
                        card.subject
                    )
                }));
                self.advance_each_opponent_scry_placements(pending, card_index + 1, events)
            }
        }
    }

    fn reject_each_opponent_scry_choice(
        &mut self,
        pending: PendingResolution,
        reason: &'static str,
    ) -> Result<RuledEventBatch, EngineError> {
        self.state.pending_resolution = Some(pending);
        Err(EngineError::Illegal(reason))
    }

    fn scry_player_is_live(&self, player: PlayerId) -> bool {
        self.state
            .player_idx(player)
            .is_some_and(|index| !self.state.players[index].has_lost)
    }

    fn current_scry_top_card(&self, card: &crate::state::PendingEachOpponentScryCard) -> bool {
        if !super::super::library_choices::current(
            self,
            card.subject,
            &[(card.object_id, card.zone_change_generation)],
        ) {
            return false;
        }
        self.state
            .player_idx(card.subject)
            .and_then(|index| self.state.players[index].library.front())
            == Some(&card.object_id)
    }

    fn configure_each_opponent_scry_prompt(
        &self,
        pending: &mut PendingResolution,
    ) -> Result<(), EngineError> {
        let source_object_id = pending.presentation.source_object_id;
        let ResolutionContinuation::EachOpponentMayScry { scry, .. } = &pending.continuation else {
            return Err(EngineError::ProgramInvariant(
                "Kaya Scry1 continuation is missing while configuring a prompt",
            ));
        };
        let (deciding_player, candidates, min, max, ordered, prompt, choice_kind) =
            match &scry.stage {
                ScryStage::OptIn {
                    subject_index,
                    delegated_chooser,
                } => {
                    let subject =
                        *scry
                            .subjects
                            .get(*subject_index)
                            .ok_or(EngineError::ProgramInvariant(
                                "Kaya Scry1 subject index is invalid",
                            ))?;
                    (
                        delegated_chooser.unwrap_or(subject),
                        Vec::new(),
                        1,
                        1,
                        false,
                        if delegated_chooser.is_some() {
                            format!("Choose whether P{subject} will scry 1.")
                        } else {
                            format!("P{subject} may scry 1 ({}).", scry.spell_label)
                        },
                        rv1::ChoiceKind::ResolutionBranch,
                    )
                }
                ScryStage::Placement {
                    card_index,
                    delegated_chooser,
                } => {
                    let card = scry
                        .cards
                        .get(*card_index)
                        .ok_or(EngineError::ProgramInvariant(
                            "Kaya Scry1 card index is invalid",
                        ))?;
                    if delegated_chooser.is_some() {
                        (
                            delegated_chooser.expect("delegated placement has a chooser"),
                            Vec::new(),
                            1,
                            1,
                            false,
                            format!(
                            "Choose whether P{}'s scry card stays on top or goes to the bottom.",
                            card.subject
                        ),
                            rv1::ChoiceKind::ResolutionBranch,
                        )
                    } else {
                        (
                            card.subject,
                            vec![card.object_id],
                            0,
                            1,
                            true,
                            "Scry 1: put the looked-at card on the bottom, or leave it on top."
                                .to_string(),
                            rv1::ChoiceKind::LibraryTop,
                        )
                    }
                }
                ScryStage::ChoosingDelegate { choice, candidates } => {
                    let subject = scry_choice_subject(scry, *choice).ok_or(
                        EngineError::ProgramInvariant("Kaya Scry1 delegate subject is missing"),
                    )?;
                    let selector = if self.scry_player_is_live(scry.controller) {
                        Some(scry.controller)
                    } else {
                        super::resolution::resolution_choice_selector(&self.state, scry.controller)
                    }
                    .ok_or(EngineError::ProgramInvariant(
                        "no live player can choose a Kaya Scry1 delegate",
                    ))?;
                    let eligible = super::resolution::resolution_choice_delegate_candidates(
                        &self.state,
                        scry.controller,
                        subject,
                    );
                    if candidates
                        .iter()
                        .flatten()
                        .all(|candidate| !eligible.contains(candidate))
                    {
                        return Err(EngineError::ProgramInvariant(
                            "Kaya Scry1 delegate menu has no eligible player",
                        ));
                    }
                    (
                        selector,
                        Vec::new(),
                        1,
                        1,
                        false,
                        format!("Choose a player to make P{subject}'s scry choice."),
                        rv1::ChoiceKind::ResolutionBranch,
                    )
                }
            };
        pending.deciding_player = deciding_player;
        pending.resolution_prompt_id = ResolutionPromptId::UNASSIGNED;
        pending.resolution_prompt_semantics = None;
        pending.presentation = PendingResolutionPresentation {
            source_object_id,
            candidates,
            min,
            max,
            ordered,
            prompt,
            choice_kind,
            unique_names: false,
        };
        Ok(())
    }

    fn append_each_opponent_scry_prompt(
        &self,
        pending: &PendingResolution,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        events.push(ev_log(pending.presentation.prompt.clone()));
        let event = self.each_opponent_scry_choice_event_for(pending)?.ok_or(
            EngineError::ProgramInvariant("Kaya Scry1 prompt event could not be built"),
        )?;
        events.push(event);
        Ok(())
    }

    fn set_each_opponent_scry_delegate(
        &self,
        pending: &mut PendingResolution,
        choice: ScryChoice,
    ) -> Result<bool, EngineError> {
        let (controller, subject) = match &pending.continuation {
            ResolutionContinuation::EachOpponentMayScry { scry, .. } => (
                scry.controller,
                scry_choice_subject(scry, choice).ok_or(EngineError::ProgramInvariant(
                    "Kaya Scry1 delegate subject is missing",
                ))?,
            ),
            _ => {
                return Err(EngineError::ProgramInvariant(
                    "Kaya Scry1 continuation is missing",
                ))
            }
        };
        let eligible = super::resolution::resolution_choice_delegate_candidates(
            &self.state,
            controller,
            subject,
        );
        let selector = if self.scry_player_is_live(controller) {
            Some(controller)
        } else {
            super::resolution::resolution_choice_selector(&self.state, controller)
        };
        if eligible.is_empty() || selector.is_none() {
            return Ok(false);
        }
        let candidates =
            super::resolution::reconcile_resolution_choice_delegate_slots(&[], &eligible);
        let ResolutionContinuation::EachOpponentMayScry { scry, .. } = &mut pending.continuation
        else {
            unreachable!("continuation was checked above")
        };
        scry.stage = ScryStage::ChoosingDelegate { choice, candidates };
        self.configure_each_opponent_scry_prompt(pending)?;
        Ok(true)
    }

    fn advance_each_opponent_scry_opt_ins(
        &mut self,
        mut pending: PendingResolution,
        mut next_index: usize,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        loop {
            let (subject_count, subject) = match &pending.continuation {
                ResolutionContinuation::EachOpponentMayScry { scry, .. } => {
                    (scry.subjects.len(), scry.subjects.get(next_index).copied())
                }
                _ => {
                    return Err(EngineError::ProgramInvariant(
                        "Kaya Scry1 continuation is missing",
                    ))
                }
            };
            if next_index >= subject_count {
                return self.capture_each_opponent_scry_cohort(pending, events);
            }
            let subject = subject.expect("index was checked against subject count");
            if self.scry_player_is_live(subject) {
                if let ResolutionContinuation::EachOpponentMayScry { scry, .. } =
                    &mut pending.continuation
                {
                    scry.stage = ScryStage::OptIn {
                        subject_index: next_index,
                        delegated_chooser: None,
                    };
                }
                self.configure_each_opponent_scry_prompt(&mut pending)?;
                self.append_each_opponent_scry_prompt(&pending, &mut events)?;
                self.state.pending_resolution = Some(pending);
                return Ok(finish_with_events(self, events));
            }
            if self.set_each_opponent_scry_delegate(
                &mut pending,
                ScryChoice::OptIn {
                    subject_index: next_index,
                },
            )? {
                self.append_each_opponent_scry_prompt(&pending, &mut events)?;
                self.state.pending_resolution = Some(pending);
                return Ok(finish_with_events(self, events));
            }
            next_index += 1;
        }
    }

    fn capture_each_opponent_scry_cohort(
        &mut self,
        mut pending: PendingResolution,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let accepted = match &pending.continuation {
            ResolutionContinuation::EachOpponentMayScry { scry, .. } => {
                scry.accepted_subjects.clone()
            }
            _ => {
                return Err(EngineError::ProgramInvariant(
                    "Kaya Scry1 continuation is missing",
                ))
            }
        };
        let mut cards = Vec::new();
        for subject in accepted {
            if !self.scry_player_is_live(subject) {
                continue;
            }
            let Some(index) = self.state.player_idx(subject) else {
                continue;
            };
            let Some(object_id) = self.state.players[index].library.front().copied() else {
                let spell_label = match &pending.continuation {
                    ResolutionContinuation::EachOpponentMayScry { scry, .. } => &scry.spell_label,
                    _ => unreachable!(),
                };
                events.push(ev_log(format!(
                    "P{subject} scries 1 with an empty library ({spell_label})."
                )));
                continue;
            };
            let generation = super::super::library_choices::capture(self, &[object_id])[0].1;
            let (_, names) = super::resolution::candidate_identities(self, &[object_id]);
            let spell_label = match &pending.continuation {
                ResolutionContinuation::EachOpponentMayScry { scry, .. } => &scry.spell_label,
                _ => unreachable!(),
            };
            events.push(ev_log(format!("P{subject} scries 1 ({spell_label}).")));
            let look = match super::super::events::ev_log_private_card_look(
                format!("P{subject} looks at {}.", names.join(", ")),
                subject,
                names,
            ) {
                Ok(look) => look,
                Err(error) => {
                    self.state.pending_resolution = Some(pending);
                    return Err(error);
                }
            };
            events.push(look);
            cards.push(crate::state::PendingEachOpponentScryCard {
                subject,
                object_id,
                zone_change_generation: generation,
                put_bottom: None,
            });
        }
        let ResolutionContinuation::EachOpponentMayScry { scry, .. } = &mut pending.continuation
        else {
            unreachable!("continuation was checked above")
        };
        scry.cards = cards;
        if scry.cards.is_empty() {
            return self.complete_each_opponent_scry(pending, events);
        }
        scry.stage = ScryStage::Placement {
            card_index: 0,
            delegated_chooser: None,
        };
        self.configure_each_opponent_scry_prompt(&mut pending)?;
        self.append_each_opponent_scry_prompt(&pending, &mut events)?;
        self.state.pending_resolution = Some(pending);
        Ok(finish_with_events(self, events))
    }

    fn advance_each_opponent_scry_placements(
        &mut self,
        mut pending: PendingResolution,
        mut next_index: usize,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        loop {
            let card = match &pending.continuation {
                ResolutionContinuation::EachOpponentMayScry { scry, .. } => {
                    scry.cards.get(next_index).cloned()
                }
                _ => {
                    return Err(EngineError::ProgramInvariant(
                        "Kaya Scry1 continuation is missing",
                    ))
                }
            };
            let Some(card) = card else {
                return self.commit_each_opponent_scry_placements(pending, events);
            };
            if self.scry_player_is_live(card.subject) {
                if let ResolutionContinuation::EachOpponentMayScry { scry, .. } =
                    &mut pending.continuation
                {
                    scry.stage = ScryStage::Placement {
                        card_index: next_index,
                        delegated_chooser: None,
                    };
                }
                self.configure_each_opponent_scry_prompt(&mut pending)?;
                self.append_each_opponent_scry_prompt(&pending, &mut events)?;
                self.state.pending_resolution = Some(pending);
                return Ok(finish_with_events(self, events));
            }
            if self.set_each_opponent_scry_delegate(
                &mut pending,
                ScryChoice::Placement {
                    card_index: next_index,
                },
            )? {
                self.append_each_opponent_scry_prompt(&pending, &mut events)?;
                self.state.pending_resolution = Some(pending);
                return Ok(finish_with_events(self, events));
            }
            if let ResolutionContinuation::EachOpponentMayScry { scry, .. } =
                &mut pending.continuation
            {
                scry.cards[next_index].put_bottom = Some(false);
            }
            next_index += 1;
        }
    }

    fn commit_each_opponent_scry_placements(
        &mut self,
        pending: PendingResolution,
        events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let cards = match &pending.continuation {
            ResolutionContinuation::EachOpponentMayScry { scry, .. } => scry.cards.clone(),
            _ => {
                return Err(EngineError::ProgramInvariant(
                    "Kaya Scry1 continuation is missing",
                ))
            }
        };
        if cards
            .iter()
            .any(|card| self.scry_player_is_live(card.subject) && !self.current_scry_top_card(card))
        {
            return self
                .reject_each_opponent_scry_choice(pending, "stale Kaya Scry1 library cohort");
        }
        for card in cards {
            if !self.scry_player_is_live(card.subject) || card.put_bottom != Some(true) {
                continue;
            }
            let index = self
                .state
                .player_idx(card.subject)
                .expect("live Scry1 subject has a seat");
            let library = &mut self.state.players[index].library;
            library.retain(|object_id| *object_id != card.object_id);
            library.push_back(card.object_id);
        }
        self.complete_each_opponent_scry(pending, events)
    }

    fn complete_each_opponent_scry(
        &mut self,
        pending: PendingResolution,
        events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        let ResolutionContinuation::EachOpponentMayScry { stack, .. } = pending.continuation else {
            return Err(EngineError::ProgramInvariant(
                "Kaya Scry1 continuation is missing",
            ));
        };
        let previous = stack.previous_result.clone();
        self.complete_parked_stack_with_previous(stack, previous, events)
    }

    pub(in crate::engine) fn refresh_each_opponent_scry_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(mut pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let stage = match &pending.continuation {
            ResolutionContinuation::EachOpponentMayScry { scry, .. } => scry.stage.clone(),
            _ => {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
        };
        let mut changed = false;
        match stage {
            ScryStage::OptIn {
                subject_index,
                delegated_chooser,
            } => {
                let subject = match &pending.continuation {
                    ResolutionContinuation::EachOpponentMayScry { scry, .. } => *scry
                        .subjects
                        .get(subject_index)
                        .ok_or(EngineError::ProgramInvariant(
                            "Kaya Scry1 subject index is invalid",
                        ))?,
                    _ => unreachable!(),
                };
                let chooser_departed =
                    delegated_chooser.is_some_and(|chooser| !self.scry_player_is_live(chooser));
                if (!self.scry_player_is_live(subject) && delegated_chooser.is_none())
                    || chooser_departed
                {
                    changed = self.set_each_opponent_scry_delegate(
                        &mut pending,
                        ScryChoice::OptIn { subject_index },
                    )?;
                }
            }
            ScryStage::Placement {
                card_index,
                delegated_chooser,
            } => {
                let subject = match &pending.continuation {
                    ResolutionContinuation::EachOpponentMayScry { scry, .. } => {
                        scry.cards.get(card_index).map(|card| card.subject).ok_or(
                            EngineError::ProgramInvariant("Kaya Scry1 card index is invalid"),
                        )?
                    }
                    _ => unreachable!(),
                };
                let chooser_departed =
                    delegated_chooser.is_some_and(|chooser| !self.scry_player_is_live(chooser));
                if (!self.scry_player_is_live(subject) && delegated_chooser.is_none())
                    || chooser_departed
                {
                    changed = self.set_each_opponent_scry_delegate(
                        &mut pending,
                        ScryChoice::Placement { card_index },
                    )?;
                }
            }
            ScryStage::ChoosingDelegate { choice, candidates } => {
                let (controller, subject) = match &pending.continuation {
                    ResolutionContinuation::EachOpponentMayScry { scry, .. } => (
                        scry.controller,
                        scry_choice_subject(scry, choice).ok_or(EngineError::ProgramInvariant(
                            "Kaya Scry1 delegate subject is missing",
                        ))?,
                    ),
                    _ => unreachable!(),
                };
                let eligible = super::resolution::resolution_choice_delegate_candidates(
                    &self.state,
                    controller,
                    subject,
                );
                let candidates = super::resolution::reconcile_resolution_choice_delegate_slots(
                    &candidates,
                    &eligible,
                );
                let selector = if self.scry_player_is_live(controller) {
                    Some(controller)
                } else {
                    super::resolution::resolution_choice_selector(&self.state, controller)
                };
                if let Some(selector) = selector {
                    changed = selector != pending.deciding_player
                        || match &pending.continuation {
                            ResolutionContinuation::EachOpponentMayScry { scry, .. } => !matches!(
                                &scry.stage,
                                ScryStage::ChoosingDelegate { candidates: previous, .. }
                                    if previous == &candidates
                            ),
                            _ => false,
                        };
                    if changed {
                        if let ResolutionContinuation::EachOpponentMayScry { scry, .. } =
                            &mut pending.continuation
                        {
                            scry.stage = ScryStage::ChoosingDelegate { choice, candidates };
                        }
                        self.configure_each_opponent_scry_prompt(&mut pending)?;
                    }
                }
            }
        }
        if changed {
            events.push(ev_log(
                "The current chooser for Kaya's Scry 1 instruction has changed.".into(),
            ));
            if let Err(error) = self.append_each_opponent_scry_prompt(&pending, events) {
                self.state.pending_resolution = Some(pending);
                return Err(error);
            }
        }
        self.state.pending_resolution = Some(pending);
        Ok(())
    }
}

fn scry_choice_subject(
    scry: &crate::state::PendingEachOpponentScry,
    choice: ScryChoice,
) -> Option<PlayerId> {
    match choice {
        ScryChoice::OptIn { subject_index } => scry.subjects.get(subject_index).copied(),
        ScryChoice::Placement { card_index } => scry.cards.get(card_index).map(|card| card.subject),
    }
}

fn scry_branch_option(branch_index: u32, label: String) -> rv1::ResolutionBranchOption {
    rv1::ResolutionBranchOption {
        branch_index,
        label: label.clone(),
        cost_kind: rv1::ResolutionBranchCostKind::Unspecified as i32,
        cost_text: String::new(),
        selectable: true,
        search_zones: Vec::new(),
        presentation: Some(rv1::PresentationRef {
            fallback_text: label,
            ..Default::default()
        }),
    }
}

fn clean_scry_branch_answer(answer: &rv1::SubmitResolutionChoice) -> bool {
    answer.chosen_object_ids.is_empty()
        && answer.chosen_player_ids.is_empty()
        && answer.cast_spell.is_none()
        && answer.chosen_combat_defender.is_none()
        && answer.payment.is_none()
        && answer.restricted_mana.is_empty()
        && answer.spell_cast_announcement.is_none()
}

fn clean_scry_object_answer(answer: &rv1::SubmitResolutionChoice) -> bool {
    answer.chosen_player_ids.is_empty()
        && answer.cast_spell.is_none()
        && answer.chosen_combat_defender.is_none()
        && answer.payment.is_none()
        && answer.restricted_mana.is_empty()
        && answer.spell_cast_announcement.is_none()
}
