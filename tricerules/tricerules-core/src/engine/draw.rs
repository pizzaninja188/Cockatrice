//! CR 121/614/616: one resumable transaction for actual draw instructions.
use super::characteristics::printed_static_source_is_available;
use super::events::{ev_log, ev_priority_changed, finish_with_events};
use super::replacement::PendingReplacementEvent;
use super::triggers::ability_definition_from;
use super::*;
use tricerules_cards::primitives::DrawReplacementCondition;

#[derive(serde::Serialize, Debug, Clone, PartialEq, Eq)]
struct DrawReplacementIdentity {
    source: ObjectId,
    generation: u64,
    definition: AbilityDefinitionId,
}

#[derive(serde::Serialize, Debug, Clone)]
struct DrawApplication {
    handle: u32,
    identity: DrawReplacementIdentity,
    action: DrawReplacementAction,
    presentation: crate::state::ReplacementSourcePresentation,
}

#[derive(serde::Serialize, Debug, Clone, Copy)]
enum DrawReplacementAction {
    Double,
    WinInstead,
}

#[derive(serde::Serialize, Debug, Clone)]
struct DrawNode {
    applied: Vec<DrawReplacementIdentity>,
}

#[derive(serde::Serialize, Debug, Clone)]
pub(crate) enum DrawCompletion {
    ResumeEffects {
        stack: ParkedStackResolution,
        result: EffectResult,
    },
    BeginDiscard {
        stack: ParkedStackResolution,
        player: PlayerId,
        count: u32,
        result: EffectResult,
    },
    FinishDrawStep {
        active_player: PlayerId,
        occurrence: u64,
    },
    BrainstormPutBack {
        stack: ParkedStackResolution,
        step: u32,
        scratch: Vec<ObjectId>,
    },
}

impl DrawCompletion {
    fn stack(&self) -> Option<&ParkedStackResolution> {
        match self {
            Self::ResumeEffects { stack, .. }
            | Self::BeginDiscard { stack, .. }
            | Self::BrainstormPutBack { stack, .. } => Some(stack),
            Self::FinishDrawStep { .. } => None,
        }
    }
    fn transfer_stack(&mut self, parent: Option<ParkedStackResolution>) {
        if let Some(parent) = parent {
            match self {
                Self::ResumeEffects { stack, .. }
                | Self::BeginDiscard { stack, .. }
                | Self::BrainstormPutBack { stack, .. } => *stack = parent,
                Self::FinishDrawStep { .. } => {}
            }
        }
    }
}

#[derive(serde::Serialize, Debug, Clone)]
struct DrawRequest {
    player: PlayerId,
    remaining: u32,
    drawn: u32,
    failed: bool,
}

#[derive(serde::Serialize, Debug, Clone)]
pub(crate) struct PendingDrawTransaction {
    requests: VecDeque<DrawRequest>,
    nodes: Vec<DrawNode>,
    applications: Vec<DrawApplication>,
    completion: DrawCompletion,
    label: String,
    receipts: Vec<TriggerObjectRef>,
}

pub(super) struct CompletedDraw {
    pub completion: DrawCompletion,
    pub receipts: Vec<TriggerObjectRef>,
}
pub(super) enum DrawProgress {
    Complete(Box<CompletedDraw>),
    Parked,
    GameEnded,
}

pub(super) fn controller_library_empty(state: &GameState, controller: PlayerId) -> bool {
    state.player_idx(controller).is_some_and(|index| {
        !state.players[index].has_lost && state.players[index].library.is_empty()
    })
}

impl GameEngine {
    fn draw_candidates(
        &self,
        drawer: PlayerId,
        node: &DrawNode,
    ) -> Vec<(DrawReplacementIdentity, DrawReplacementAction)> {
        let first_own_step = self.state.turn_step == TurnStep::Draw
            && self.state.active_player_id() == drawer
            && self
                .state
                .draw_step_progress
                .is_some_and(|(player, _, count)| player == drawer && count == 0);
        let mut sources = self.state.objects.keys().copied().collect::<Vec<_>>();
        sources.sort_unstable();
        let mut result = Vec::new();
        for source in sources {
            if !printed_static_source_is_available(&self.state, self.registry, source)
                || self
                    .characteristics(source)
                    .is_none_or(|value| value.controller != drawer)
            {
                continue;
            }
            let Some(face) = self.effective_face(source) else {
                continue;
            };
            for ability in &face.static_abilities {
                let action = match ability.definition {
                    StaticAbilityDef::DoubleControllerDraws { condition } => {
                        if condition
                            == DrawReplacementCondition::ExceptFirstSuccessfulDrawInOwnDrawStep
                            && first_own_step
                        {
                            continue;
                        }
                        DrawReplacementAction::Double
                    }
                    StaticAbilityDef::WinControllerInsteadOfEmptyLibraryDraw
                        if controller_library_empty(&self.state, drawer) =>
                    {
                        DrawReplacementAction::WinInstead
                    }
                    _ => continue,
                };
                let identity = DrawReplacementIdentity {
                    source,
                    generation: self
                        .state
                        .zone_change_generation
                        .get(&source)
                        .copied()
                        .unwrap_or(0),
                    definition: ability_definition_from(
                        &self.state,
                        self.registry,
                        source,
                        self.state.objects[&source].face_up_index,
                        vec![ability.ability_id.clone()],
                    ),
                };
                if !node.applied.contains(&identity) {
                    result.push((identity, action));
                }
            }
        }
        result
    }

    pub(super) fn start_draw_transaction(
        &mut self,
        requests: Vec<(PlayerId, u32)>,
        completion: DrawCompletion,
        label: &str,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<DrawProgress, EngineError> {
        self.advance_draw_transaction(
            PendingDrawTransaction {
                requests: requests
                    .into_iter()
                    .map(|(player, remaining)| DrawRequest {
                        player,
                        remaining,
                        drawn: 0,
                        failed: false,
                    })
                    .collect(),
                nodes: Vec::new(),
                applications: Vec::new(),
                completion,
                label: label.to_string(),
                receipts: Vec::new(),
            },
            events,
        )
    }

    fn advance_draw_transaction(
        &mut self,
        mut work: PendingDrawTransaction,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<DrawProgress, EngineError> {
        if self.state.is_terminal() {
            return Ok(DrawProgress::GameEnded);
        }
        while let Some(request) = work.requests.front_mut() {
            let Some(index) = self
                .state
                .player_idx(request.player)
                .filter(|index| !self.state.players[*index].has_lost)
            else {
                work.requests.pop_front();
                work.nodes.clear();
                continue;
            };
            if work.nodes.is_empty() {
                if request.remaining == 0 {
                    events.push(ev_log(format!(
                        "P{} draws {} {} ({}).",
                        request.player,
                        request.drawn,
                        if request.drawn == 1 { "card" } else { "cards" },
                        work.label
                    )));
                    if request.failed {
                        events.push(ev_log(format!("P{} attempted to draw more cards than remained in the library; loss pending until the next state-based-action check (CR 121.4/704.5b; CR 704.4).", request.player)));
                    }
                    work.requests.pop_front();
                    continue;
                }
                request.remaining -= 1;
                work.nodes.push(DrawNode {
                    applied: Vec::new(),
                });
            }
            let node = work.nodes.last().expect("current draw node");
            let candidates = self.draw_candidates(request.player, node);
            if candidates.len() > 1 {
                work.applications.clear();
                for (identity, action) in candidates {
                    let handle = self.state.next_replacement_application_id;
                    self.state.next_replacement_application_id = handle.checked_add(1).ok_or(
                        EngineError::Illegal("replacement application ids exhausted"),
                    )?;
                    work.applications.push(DrawApplication {
                        handle,
                        presentation: self.replacement_source_presentation(identity.source),
                        identity,
                        action,
                    });
                }
                let player = request.player;
                let stack = work.completion.stack().cloned();
                let source_object_id = stack.as_ref().map_or(0, |stack| stack.item.id);
                let candidates = work.applications.iter().map(|value| value.handle).collect();
                self.state.pending_replacement_event =
                    Some(PendingReplacementEvent::Draw(Box::new(work)));
                self.state.pending_resolution = Some(PendingResolution {
                    deciding_player: player,
                    presentation: PendingResolutionPresentation {
                        source_object_id,
                        candidates,
                        min: 1,
                        max: 1,
                        ordered: false,
                        unique_names: false,
                        prompt: "Choose which draw replacement applies next.".to_string(),
                        choice_kind: rv1::ChoiceKind::ReplacementEffect,
                    },
                    continuation: ResolutionContinuation::DrawReplacement { stack },
                });
                events.push(
                    self.draw_replacement_choice_event()
                        .expect("parked draw choice"),
                );
                return Ok(DrawProgress::Parked);
            }
            if let Some((identity, action)) = candidates.into_iter().next() {
                if self.apply_draw_replacement(&mut work, identity, action) {
                    return Ok(DrawProgress::GameEnded);
                }
                continue;
            }
            work.nodes.pop().expect("current draw node");
            if let Some(oid) = self.state.players[index].library.front().copied() {
                resolution::draw_card(&mut self.state, self.registry, request.player)?;
                if self.state.turn_step == TurnStep::Draw
                    && self.state.active_player_id() == request.player
                {
                    if let Some((player, _, count)) = self.state.draw_step_progress.as_mut() {
                        if *player == request.player {
                            *count = count.saturating_add(1);
                        }
                    }
                }
                work.receipts.push(TriggerObjectRef {
                    object_id: oid,
                    zone_change_generation: self.state.zone_change_generation[&oid],
                    controller_at_event: request.player,
                });
                request.drawn += 1;
                self.fire_card_drawn(request.player);
            } else {
                self.state.players[index].pending_library_loss = true;
                request.failed = true;
            }
        }
        Ok(DrawProgress::Complete(Box::new(CompletedDraw {
            completion: work.completion,
            receipts: work.receipts,
        })))
    }

    fn apply_draw_replacement(
        &mut self,
        work: &mut PendingDrawTransaction,
        identity: DrawReplacementIdentity,
        action: DrawReplacementAction,
    ) -> bool {
        let mut node = work.nodes.pop().expect("current draw node");
        node.applied.push(identity);
        match action {
            DrawReplacementAction::Double => {
                // Children inherit ancestors; a subsequently modified sibling never changes them.
                work.nodes.push(node.clone());
                work.nodes.push(node);
                false
            }
            DrawReplacementAction::WinInstead => {
                // CR 614.6: consume the draw before attempting the replacement's win. Even a
                // future prohibited win cannot restore this node or create a failed draw.
                self.state
                    .outcome
                    .get_or_insert(crate::state::GameOutcome::Winner(
                        work.requests.front().expect("drawer").player,
                    ));
                true
            }
        }
    }

    pub(super) fn draw_replacement_choice_event(&self) -> Option<rv1::RuledEvent> {
        let pending = self.state.pending_resolution.as_ref()?;
        let Some(PendingReplacementEvent::Draw(work)) = &self.state.pending_replacement_event
        else {
            return None;
        };
        Some(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: pending.presentation.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ReplacementEffect as i32,
                    candidate_object_ids: pending.presentation.candidates.clone(),
                    min: 1,
                    max: 1,
                    candidate_selectable: vec![true; work.applications.len()],
                    replacement_options: work
                        .applications
                        .iter()
                        .map(|value| {
                            value.presentation.option(
                                value.handle,
                                match value.action {
                                    DrawReplacementAction::Double => "Draw two cards instead.",
                                    DrawReplacementAction::WinInstead => "Win the game instead.",
                                }
                                .to_string(),
                            )
                        })
                        .collect(),
                    ..Default::default()
                },
            )),
        })
    }

    pub(super) fn finish_draw_replacement_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<RuledEventBatch, EngineError> {
        let Some(PendingReplacementEvent::Draw(work)) = &self.state.pending_replacement_event
        else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("draw replacement event missing"));
        };
        let selected = (decision == rv1::ResolutionChoiceDecision::Unspecified
            && answer.chosen_object_ids.len() == 1
            && answer.chosen_player_ids.is_empty()
            && answer.selected_branch_index == 0
            && answer.cast_spell.is_none()
            && answer.chosen_combat_defender.is_none()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty()
            && answer.spell_cast_announcement.is_none())
        .then(|| {
            work.applications
                .iter()
                .find(|value| value.handle == answer.chosen_object_ids[0])
        })
        .flatten();
        let Some(selected) = selected else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "choose one published draw replacement",
            ));
        };
        let current =
            work.requests
                .front()
                .zip(work.nodes.last())
                .is_some_and(|(request, node)| {
                    self.draw_candidates(request.player, node)
                        .iter()
                        .any(|(identity, _)| identity == &selected.identity)
                });
        if !current {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("draw replacement source is stale"));
        }
        let identity = selected.identity.clone();
        let action = selected.action;
        let Some(PendingReplacementEvent::Draw(work)) = self.state.pending_replacement_event.take()
        else {
            unreachable!()
        };
        let mut work = *work;
        let ResolutionContinuation::DrawReplacement { stack } = pending.continuation else {
            unreachable!()
        };
        work.completion.transfer_stack(stack);
        let mut events = Vec::new();
        if self.apply_draw_replacement(&mut work, identity, action) {
            return Ok(finish_with_events(self, events));
        }
        match self.advance_draw_transaction(work, &mut events)? {
            DrawProgress::GameEnded => Ok(finish_with_events(self, events)),
            DrawProgress::Parked => Ok(finish_with_events(self, events)),
            DrawProgress::Complete(completed) => self.complete_draw_transaction(completed, events),
        }
    }

    pub(super) fn complete_draw_transaction(
        &mut self,
        completed: Box<CompletedDraw>,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<RuledEventBatch, EngineError> {
        match completed.completion {
            DrawCompletion::ResumeEffects { stack, mut result } => {
                result.produced_objects.extend(completed.receipts);
                self.complete_parked_resolution_with_previous(
                    stack.item,
                    stack.resume_effect_index,
                    result,
                    events,
                )
            }
            DrawCompletion::BeginDiscard {
                stack,
                player,
                count,
                mut result,
            } => {
                result.produced_objects.extend(completed.receipts);
                resolution::zones::resume_draw_then_discard(
                    self, stack, player, count, result, events,
                )
            }
            DrawCompletion::FinishDrawStep {
                active_player,
                occurrence,
            } => {
                self.finish_draw_step_action(active_player, occurrence, &mut events)?;
                self.apply_sbas(&mut events)?;
                Ok(finish_with_events(self, events))
            }
            DrawCompletion::BrainstormPutBack {
                stack,
                step,
                scratch,
            } => {
                self.finish_brainstorm_draw(stack.item, step, scratch, &mut events)?;
                if self.state.pending_resolution.is_none() {
                    self.apply_sbas(&mut events)?;
                    events.push(ev_priority_changed(self));
                }
                Ok(finish_with_events(self, events))
            }
        }
    }

    /// An accepted concession can remove the drawer, one replacement source, or the resolving
    /// spell itself. Reevaluate the parked node without applying the vanished choice handle.
    pub(super) fn reconcile_draw_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        if !matches!(
            self.state.pending_replacement_event,
            Some(PendingReplacementEvent::Draw(_))
        ) {
            return Ok(());
        }
        let pending = self.state.pending_resolution.take();
        let Some(PendingReplacementEvent::Draw(work)) = self.state.pending_replacement_event.take()
        else {
            unreachable!()
        };
        let Some(pending) = pending.filter(|_| !self.state.is_terminal()) else {
            return Ok(());
        };
        let ResolutionContinuation::DrawReplacement { stack } = pending.continuation else {
            unreachable!()
        };
        let mut work = *work;
        work.completion.transfer_stack(stack);
        work.applications.clear();
        match self.advance_draw_transaction(work, events)? {
            DrawProgress::GameEnded => {}
            DrawProgress::Parked => {}
            DrawProgress::Complete(done) => {
                let batch = self.complete_draw_transaction(done, std::mem::take(events))?;
                *events = batch.events;
            }
        }
        Ok(())
    }

    pub(super) fn finish_draw_step_action(
        &mut self,
        active_player: PlayerId,
        occurrence: u64,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        // Automatic priority settlement bypasses the ordinary command boundary. The
        // turn-based draw has finished, so perform its deferred loss before publishing
        // draw-step triggers or permitting another automatic pass (CR 504.2/704.5b).
        if self
            .state
            .players
            .iter()
            .any(|player| player.id == active_player && player.pending_library_loss)
        {
            events.push(ev_log(if self.state.players.len() == 2 {
                "Game over: empty library on draw".to_string()
            } else {
                format!("P{active_player} loses: empty library on draw")
            }));
        }
        self.commit_pending_library_losses();
        self.sweep_life();
        self.reconcile_departed_players(events)?;
        if self
            .state
            .draw_step_progress
            .is_some_and(|(player, step, _)| player == active_player && step == occurrence)
            && self
                .state
                .players
                .iter()
                .any(|player| player.id == active_player && !player.has_lost)
        {
            self.state.passes_since_stack_change = 0;
            self.fire_triggers(&[GameEvent::PhaseBegan {
                phase: rv1::PhaseId::Draw,
                active_player,
            }]);
            self.flush_staged_triggers(events);
            if self.state.blocking_choice().is_none() {
                events.push(ev_priority_changed(self));
            }
        }
        Ok(())
    }
}
