//! Player-authorized priority shortcuts. Every automatic action still enters normal dispatch.
//! ETB and upkeep triggers share TriggerUseKey; presentation strings never identify a rule.
use super::*;
use rv1::{
    AutomationAnswer as Answer, AutomationTriggerScope as Scope, AutomationYieldMode as Mode,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct Policy {
    revision: u64,
    full_control: bool,
    auto_pass: bool,
    paused: bool,
    yielding: Option<Yield>,
    stop_once: Option<rv1::AutomationPhaseStop>,
    rules: Vec<Rule>,
    next_rule: u64,
}

#[derive(Clone, Debug)]
struct Yield {
    mode: Mode,
    turn: u64,
    stack: Option<rv1::AutomationStackRef>,
    initial_stack: Vec<rv1::AutomationStackRef>,
    phase: Option<rv1::AutomationPhaseStop>,
}

#[derive(Clone, Debug)]
struct Rule {
    id: u64,
    key: TriggerUseKey,
    controller: PlayerId,
    scope: Scope,
    yield_priority: bool,
    answer: Answer,
    description: String,
}

// Include policy in diagnostic/replay state hashes. Wire messages intentionally do not derive
// serde; these ordered scalar tuples serialize their complete, small internal state.
impl serde::Serialize for Policy {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let stack_ref = |r: &rv1::AutomationStackRef| (r.object_id, r.zone_change_generation);
        let phase = |p: &rv1::AutomationPhaseStop| (p.phase, p.on_own_turn);
        let yielding = self.yielding.as_ref().map(|y| {
            (
                y.mode as i32,
                y.turn,
                y.stack.as_ref().map(stack_ref),
                y.initial_stack.iter().map(stack_ref).collect::<Vec<_>>(),
                y.phase.as_ref().map(phase),
            )
        });
        let rules: Vec<_> = self
            .rules
            .iter()
            .map(|r| {
                (
                    r.id,
                    &r.key,
                    r.controller,
                    r.scope as i32,
                    r.yield_priority,
                    r.answer as i32,
                    &r.description,
                )
            })
            .collect();
        let mut state = serializer.serialize_struct("AutomationPolicy", 8)?;
        state.serialize_field("revision", &self.revision)?;
        state.serialize_field("full_control", &self.full_control)?;
        state.serialize_field("auto_pass", &self.auto_pass)?;
        state.serialize_field("paused", &self.paused)?;
        state.serialize_field("yielding", &yielding)?;
        state.serialize_field("stop_once", &self.stop_once.as_ref().map(phase))?;
        state.serialize_field("rules", &rules)?;
        state.serialize_field("next_rule", &self.next_rule)?;
        state.end()
    }
}

impl Rule {
    fn matches(&self, item: &StackItem) -> bool {
        item.is_triggered
            && item.controller == self.controller
            && item.trigger_use_key.as_ref().is_some_and(|key| {
                key.ability_origin == self.key.ability_origin
                    && (self.scope == Scope::MatchingAbility || *key == self.key)
            })
    }
}

impl GameEngine {
    fn automation_stack_ref(&self, item: &StackItem) -> rv1::AutomationStackRef {
        rv1::AutomationStackRef {
            object_id: item.id,
            zone_change_generation: (!item.is_copy && item.ability_text.is_none()).then(|| {
                self.state
                    .zone_change_generation
                    .get(&item.id)
                    .copied()
                    .unwrap_or(0)
            }),
        }
    }

    fn automation_stack_item(&self, reference: &rv1::AutomationStackRef) -> Option<&StackItem> {
        self.automation_stack_items()
            .find(|item| self.automation_stack_ref(item) == *reference)
    }

    fn automation_stack_items(&self) -> impl Iterator<Item = &StackItem> {
        self.state.stack.iter().chain(
            self.state
                .pending_resolution
                .as_ref()
                .and_then(|pending| pending.continuation.stack())
                .map(|stack| &stack.item)
                .filter(|item| !self.state.stack.iter().any(|current| current.id == item.id)),
        )
    }

    fn automation_source_current(&self, key: &TriggerUseKey) -> bool {
        self.state.objects.contains_key(&key.object_id)
            && self
                .state
                .zone_change_generation
                .get(&key.object_id)
                .copied()
                .unwrap_or(0)
                == key.zone_change_generation
    }

    fn automation_phase_matches(&self, player: PlayerId, stop: &rv1::AutomationPhaseStop) -> bool {
        stop.phase == self.current_phase_id() as i32
            && stop.on_own_turn == (player == self.state.active_player_id())
    }

    fn validate_automation_phase(
        &self,
        stop: &rv1::AutomationPhaseStop,
    ) -> Result<(), EngineError> {
        Self::validate_auto_pass_stop_list(&[stop.phase])
    }

    pub(super) fn update_automation(
        &mut self,
        player: PlayerId,
        change: &rv1::UpdateAutomation,
    ) -> Result<RuledEventBatch, EngineError> {
        use rv1::update_automation::Action;
        let mut policy = self
            .state
            .automation
            .get(&player)
            .cloned()
            .unwrap_or_default();
        match change
            .action
            .as_ref()
            .ok_or(EngineError::Illegal("empty automation update"))?
        {
            Action::ClearAll(true) => {
                policy.rules.clear();
                policy.auto_pass = false;
                policy.full_control = true;
                policy.paused = false;
                policy.yielding = None;
                policy.stop_once = None;
            }
            Action::AutoPass(enabled) => {
                policy.auto_pass = *enabled;
                policy.yielding = None;
                policy.paused = false;
                if *enabled {
                    policy.full_control = false;
                }
            }
            Action::FullControl(enabled) => {
                policy.full_control = *enabled;
                policy.paused = false;
                policy.yielding = None;
            }
            Action::Pause(true) => {
                policy.paused = true;
                policy.yielding = None;
            }
            Action::YieldTo(request) => {
                let mode = Mode::try_from(request.mode)
                    .map_err(|_| EngineError::Illegal("unknown yield mode"))?;
                if mode == Mode::None || !self.automation_priority_window() {
                    return Err(EngineError::Illegal(
                        "yield requires an ordinary priority window",
                    ));
                }
                if (mode == Mode::StackItem) != request.stack.is_some()
                    || (mode == Mode::Phase) != request.phase.is_some()
                {
                    return Err(EngineError::Illegal(
                        "yield has unrelated or missing fields",
                    ));
                }
                if let Some(reference) = &request.stack {
                    self.automation_stack_item(reference)
                        .ok_or(EngineError::Illegal("stale stack yield"))?;
                }
                if matches!(mode, Mode::CurrentStack | Mode::EntireStack)
                    && self.state.stack.is_empty()
                {
                    return Err(EngineError::Illegal("stack yield requires a stack item"));
                }
                if let Some(phase) = &request.phase {
                    self.validate_automation_phase(phase)?;
                }
                policy.yielding = Some(Yield {
                    mode,
                    turn: self.state.turn_instance,
                    stack: request.stack,
                    initial_stack: self
                        .state
                        .stack
                        .iter()
                        .map(|item| self.automation_stack_ref(item))
                        .collect(),
                    phase: request.phase,
                });
                policy.full_control = false;
                policy.paused = false;
            }
            Action::StopOnce(stop) => {
                self.validate_automation_phase(stop)?;
                policy.stop_once = Some(*stop);
            }
            Action::TriggerRule(request) => {
                let reference = request
                    .stack
                    .as_ref()
                    .ok_or(EngineError::Illegal("missing trigger"))?;
                let item = self
                    .automation_stack_item(reference)
                    .ok_or(EngineError::Illegal("stale trigger"))?;
                let key = item
                    .trigger_use_key
                    .clone()
                    .filter(|_| item.is_triggered)
                    .ok_or(EngineError::Illegal(
                        "stack item has no reusable trigger identity",
                    ))?;
                let scope = Scope::try_from(request.scope)
                    .map_err(|_| EngineError::Illegal("unknown trigger scope"))?;
                if scope == Scope::Source && !self.automation_source_current(&key) {
                    return Err(EngineError::Illegal("trigger source incarnation has left"));
                }
                let answer = Answer::try_from(request.answer)
                    .map_err(|_| EngineError::Illegal("unknown trigger answer"))?;
                if answer != Answer::Ask
                    && (item.controller != player || !Self::automation_optional_trigger(item))
                {
                    return Err(EngineError::Illegal(
                        "trigger has no automatable optional choice for this player",
                    ));
                }
                if !request.yield_priority && answer == Answer::Ask {
                    return Err(EngineError::Illegal("empty trigger rule"));
                }
                let existing = policy
                    .rules
                    .iter()
                    .position(|rule| rule.scope == scope && rule.matches(item));
                let id = if let Some(index) = existing {
                    policy.rules.remove(index).id
                } else {
                    if policy.rules.len() >= 256 {
                        return Err(EngineError::Illegal("too many trigger rules"));
                    }
                    policy.next_rule = policy
                        .next_rule
                        .checked_add(1)
                        .ok_or(EngineError::Illegal("automation rule ids exhausted"))?;
                    policy.next_rule
                };
                // The label is already public on the selected stack occurrence. It has no
                // matching role, and remains useful after its source has disappeared.
                let description = item.ability_text.clone().unwrap_or_default();
                policy.rules.push(Rule {
                    id,
                    key,
                    controller: item.controller,
                    scope,
                    yield_priority: request.yield_priority,
                    answer,
                    description,
                });
                policy.full_control = false;
                policy.paused = false;
            }
            Action::RemoveRule(id) => {
                let before = policy.rules.len();
                policy.rules.retain(|rule| rule.id != *id);
                if policy.rules.len() == before {
                    return Err(EngineError::Illegal("unknown automation rule"));
                }
            }
            Action::ClearAll(false) | Action::Pause(false) => {
                return Err(EngineError::Illegal("automation action must be enabled"));
            }
        }
        policy.revision = policy
            .revision
            .checked_add(1)
            .ok_or(EngineError::Illegal("automation revision exhausted"))?;
        self.state.automation.insert(player, policy);
        let mut batch = RuledEventBatch::default();
        legal_actions::fill_legal(&mut batch, self);
        Ok(batch)
    }

    fn automation_optional_trigger(item: &StackItem) -> bool {
        item.is_triggered
            && item
                .triggered_ability
                .as_ref()
                .is_some_and(|ability| ability.may)
    }

    pub(super) fn automation_priority_window(&self) -> bool {
        !self.state.is_terminal()
            && self.state.opening.is_none()
            && self.state.blocking_choice().is_none()
            && self.state.pending_spell_cast.is_none()
            && self.state.pending_ability_activation.is_none()
            && self.state.pending_attack_declaration.is_none()
            && self.state.cleanup_discard_player.is_none()
            && !super::combat::priority_locked_for_combat_declaration(&self.state)
            && !self
                .state
                .combat
                .as_ref()
                .is_some_and(|combat| combat.assign_combat_damage_phase)
    }

    /// Only the semantic optional-effect gate is eligible, never target selection or payment.
    fn automation_binary_choice(&self, player: PlayerId) -> Option<(&StackItem, bool)> {
        let pending = self.state.pending_resolution.as_ref()?;
        if pending.deciding_player != player || pending.resolution_prompt_id.get() == 0 {
            return None;
        }
        let stack = pending.continuation.stack()?;
        let selectable = match &pending.continuation {
            ResolutionContinuation::OptionalTriggeredAbility {
                stage: crate::state::PendingOptionalTriggeredAbilityStage::Selecting,
                ..
            } => {
                super::resolution::direct_draw_effect_is_selectable(
                    self,
                    &stack.item,
                    stack.previous_result(),
                    &stack.item.triggered_ability.as_ref()?.effect,
                ) != Some(false)
            }
            ResolutionContinuation::AuthoredBranch { branch, .. }
                if branch.optional
                    && branch.branches.len() == 1
                    && matches!(
                        branch.stage,
                        crate::state::PendingResolutionBranchStage::Selecting
                    )
                    && matches!(branch.branches[0].cost, ResolutionCost::None) =>
            {
                super::resolution::resolution_branch_is_selectable(
                    self,
                    &stack.item,
                    stack.previous_result(),
                    player,
                    stack.item.controller,
                    &branch.branches[0],
                    true,
                )
            }
            _ => return None,
        };
        Some((&stack.item, selectable))
    }

    fn automation_answer(&self, player: PlayerId, policy: &Policy) -> Option<RuledCommand> {
        if !matches!(
            self.state.pending_resolution.as_ref()?.continuation,
            ResolutionContinuation::OptionalTriggeredAbility {
                stage: crate::state::PendingOptionalTriggeredAbilityStage::Selecting,
                ..
            }
        ) {
            return None;
        }
        let (item, selectable) = self.automation_binary_choice(player)?;
        let rule = policy
            .rules
            .iter()
            .filter(|rule| rule.matches(item) && rule.answer != Answer::Ask)
            .min_by_key(|rule| (rule.scope != Scope::Source, rule.id))?;
        if rule.answer == Answer::Yes && !selectable {
            return None;
        }
        Some(RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: self
                        .state
                        .pending_resolution
                        .as_ref()?
                        .resolution_prompt_id
                        .get(),
                    decision: if rule.answer == Answer::Yes {
                        rv1::ResolutionChoiceDecision::SelectBranch
                    } else {
                        rv1::ResolutionChoiceDecision::Decline
                    } as i32,
                    ..Default::default()
                },
            )),
        })
    }

    /// Return the next authorized action, or nothing. Even a caller with a forged continuation
    /// can only execute this decision, using the current rules state and exact prompt identity.
    fn automation_action(
        &self,
        player: PlayerId,
        stops: &[rv1::AutoPassPolicy],
        actions: Option<&rv1::LegalActions>,
    ) -> Option<RuledCommand> {
        let policy = self.state.automation.get(&player)?;
        if self.state.is_terminal() || policy.full_control || policy.paused {
            return None;
        }
        if let Some(answer) = self.automation_answer(player, policy) {
            return Some(answer);
        }
        if !self.automation_priority_window() || self.state.priority_player_id() != player {
            return None;
        }
        let pass = || RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
        };
        if let Some(yielding) = &policy.yielding {
            return match yielding.mode {
                Mode::StackItem
                    if self
                        .state
                        .stack
                        .last()
                        .map(|item| self.automation_stack_ref(item))
                        != yielding.stack =>
                {
                    None
                }
                Mode::None => None,
                _ => Some(pass()),
            };
        }
        if self.state.stack.last().is_some_and(|item| {
            policy
                .rules
                .iter()
                .any(|rule| rule.yield_priority && rule.matches(item))
        }) {
            return Some(pass());
        }
        let stop = stops.iter().find(|stop| stop.player_id == player)?;
        if self.policy_stops_at_current_phase(stop) {
            return None;
        }
        if policy.auto_pass {
            return actions
                .filter(|actions| {
                    actions.automation.as_ref().is_some_and(|state| {
                        state.stop_reason == rv1::AutomationStopReason::None as i32
                    })
                })
                .map(|_| pass());
        }
        self.state.stack.is_empty().then(pass)
    }

    pub(super) fn continue_automation(
        &mut self,
        player: PlayerId,
        request: &rv1::ContinueAutomation,
        stops: &[rv1::AutoPassPolicy],
    ) -> Result<RuledEventBatch, EngineError> {
        if request.expected_command_index != self.state.command_index
            || self
                .state
                .automation
                .get(&player)
                .is_none_or(|policy| policy.revision != request.expected_revision)
        {
            return Err(EngineError::Illegal("stale automation continuation"));
        }
        let mut offers = RuledEventBatch::default();
        legal_actions::fill_legal(&mut offers, self);
        let command = self
            .automation_action(player, stops, offers.legal_by_player.get(&player))
            .ok_or(EngineError::Illegal("no automatic action is authorized"))?;
        self.dispatch_command(player, &command)
    }

    pub(super) fn automation_snapshot(&self, player: PlayerId) -> rv1::AutomationState {
        let default_policy = Policy::default();
        let policy = self
            .state
            .automation
            .get(&player)
            .unwrap_or(&default_policy);
        let binary = self.automation_binary_choice(player);
        rv1::AutomationState {
            stop_reason: rv1::AutomationStopReason::Uncertain as i32,
            revision: policy.revision,
            full_control: policy.full_control,
            auto_pass: policy.auto_pass,
            paused: policy.paused,
            yield_mode: policy
                .yielding
                .as_ref()
                .map_or(Mode::None, |yielding| yielding.mode) as i32,
            yield_phase: policy.yielding.as_ref().and_then(|yielding| yielding.phase),
            stop_once: policy.stop_once,
            command_index: self.state.command_index,
            yes_branch_index: binary.filter(|(_, selectable)| *selectable).map(|_| 0),
            can_answer_no: binary.is_some(),
            can_start_yield: self.automation_priority_window(),
            rules: policy
                .rules
                .iter()
                .map(|rule| rv1::AutomationRuleInfo {
                    rule_id: rule.id,
                    description: rule.description.clone(),
                    scope: rule.scope as i32,
                    yield_priority: rule.yield_priority,
                    answer: rule.answer as i32,
                    controller_player_id: rule.controller,
                })
                .collect(),
            stack_offers: self
                .automation_stack_items()
                .map(|item| rv1::StackAutomationOffer {
                    stack: Some(self.automation_stack_ref(item)),
                    can_yield_trigger: item.is_triggered && item.trigger_use_key.is_some(),
                    can_source_rule: item.is_triggered
                        && item
                            .trigger_use_key
                            .as_ref()
                            .is_some_and(|key| self.automation_source_current(key)),
                    can_answer_optional: item.controller == player
                        && Self::automation_optional_trigger(item)
                        && item.trigger_use_key.is_some(),
                    source_rule_id: policy
                        .rules
                        .iter()
                        .find(|rule| rule.scope == Scope::Source && rule.matches(item))
                        .map_or(0, |rule| rule.id),
                    matching_rule_id: policy
                        .rules
                        .iter()
                        .find(|rule| rule.scope == Scope::MatchingAbility && rule.matches(item))
                        .map_or(0, |rule| rule.id),
                })
                .collect(),
        }
    }

    pub(super) fn publish_automation(
        &self,
        stops: &[rv1::AutoPassPolicy],
        batch: &mut RuledEventBatch,
    ) {
        batch.automation_continuation = self.state.players.iter().find_map(|player| {
            if player.has_lost
                || self
                    .automation_action(player.id, stops, batch.legal_by_player.get(&player.id))
                    .is_none()
            {
                return None;
            }
            Some(rv1::AutomationContinuation {
                player_id: player.id,
                expected_command_index: self.state.command_index,
                expected_revision: self.state.automation[&player.id].revision,
            })
        });
        for (&player, actions) in &mut batch.legal_by_player {
            let availability = actions
                .automation
                .as_ref()
                .map_or(rv1::AutomationStopReason::Uncertain as i32, |state| {
                    state.stop_reason
                });
            let mut snapshot = self.automation_snapshot(player);
            snapshot.stop_reason = if batch
                .automation_continuation
                .as_ref()
                .is_some_and(|token| token.player_id == player)
            {
                rv1::AutomationStopReason::None as i32
            } else if !self.automation_priority_window() {
                rv1::AutomationStopReason::RequiredChoice as i32
            } else if self.state.priority_player_id() != player {
                rv1::AutomationStopReason::WaitingForPriority as i32
            } else if let Some(policy) = stops.iter().find(|policy| policy.player_id == player) {
                if self.policy_stops_at_current_phase(policy) {
                    rv1::AutomationStopReason::PhaseStop as i32
                } else {
                    availability
                }
            } else {
                rv1::AutomationStopReason::MissingPolicy as i32
            };
            actions.automation = Some(snapshot);
        }
    }

    pub(super) fn reconcile_automation(
        &mut self,
        actor: PlayerId,
        command: &RuledCommand,
        batch: &RuledEventBatch,
    ) {
        use rv1::ruled_command::Cmd;
        let automated = matches!(
            command.cmd,
            Some(Cmd::ContinueAutomation(_) | Cmd::UpdateAutomation(_))
        );
        let current_stack: Vec<_> = self
            .state
            .stack
            .iter()
            .map(|item| self.automation_stack_ref(item))
            .collect();
        let players: Vec<_> = self.state.automation.keys().copied().collect();
        for player in players {
            let mut policy = self.state.automation[&player].clone();
            let mut changed = false;
            let before = policy.rules.len();
            policy.rules.retain(|rule| {
                rule.scope == Scope::MatchingAbility
                    || self
                        .state
                        .zone_change_generation
                        .get(&rule.key.object_id)
                        .copied()
                        .unwrap_or(0)
                        == rule.key.zone_change_generation
            });
            changed |= before != policy.rules.len();
            if !automated && player == actor && (policy.yielding.is_some() || policy.paused) {
                policy.yielding = None;
                // Answering a required choice never resumes yielding. A deliberate Pass
                // resumes saved rules, without restoring the discarded transient yield.
                policy.paused = !matches!(command.cmd, Some(Cmd::PassPriority(_)));
                changed = true;
            }
            if policy
                .stop_once
                .as_ref()
                .is_some_and(|stop| self.automation_phase_matches(player, stop))
            {
                policy.stop_once = None;
                policy.yielding = None;
                policy.paused = true;
                changed = true;
            }
            if let Some(yielding) = &policy.yielding {
                let expired = match yielding.mode {
                    Mode::None => true,
                    Mode::Turn | Mode::UntilResponse => self.state.turn_instance != yielding.turn,
                    Mode::EndStep => {
                        self.state.turn_instance != yielding.turn
                            || self.current_phase_id() == rv1::PhaseId::EndStep
                    }
                    Mode::StackItem => yielding
                        .stack
                        .as_ref()
                        .is_none_or(|reference| !current_stack.contains(reference)),
                    Mode::CurrentStack | Mode::EntireStack => {
                        current_stack.is_empty() && self.state.pending_resolution.is_none()
                    }
                    Mode::Phase => yielding
                        .phase
                        .as_ref()
                        .is_some_and(|stop| self.automation_phase_matches(player, stop)),
                };
                let interrupted = matches!(
                    yielding.mode,
                    Mode::CurrentStack | Mode::UntilResponse | Mode::EndStep | Mode::Phase
                ) && current_stack.iter().any(|reference| {
                    !yielding.initial_stack.contains(reference)
                        && (yielding.mode == Mode::CurrentStack
                            || self.automation_stack_item(reference).is_some_and(|item| {
                                !policy
                                    .rules
                                    .iter()
                                    .any(|rule| rule.yield_priority && rule.matches(item))
                            }))
                });
                if expired || interrupted {
                    policy.yielding = None;
                    policy.paused = true;
                    changed = true;
                }
            }
            // Required decisions stop yields before any submitted answer can resume them.
            // A specifically configured, legal optional answer is the sole exception.
            // A preference can be changed while an existing choice is open, without the
            // engine republishing its prompt event. Inspect parked state as well as new events.
            let parked_choice = self
                .state
                .pending_resolution
                .as_ref()
                .is_some_and(|pending| pending.deciding_player == player)
                || self
                    .state
                    .pending_trigger_order
                    .as_ref()
                    .is_some_and(|pending| pending.deciding_player == player)
                || self
                    .state
                    .pending_triggers
                    .iter()
                    .any(|pending| pending.controller == player)
                || self
                    .state
                    .pending_spell_cast
                    .as_ref()
                    .is_some_and(|pending| pending.caster == player)
                || self
                    .state
                    .pending_ability_activation
                    .as_ref()
                    .is_some_and(|pending| pending.deciding_player_id == player)
                || self
                    .state
                    .pending_attack_declaration
                    .as_ref()
                    .is_some_and(|pending| pending.attacking_player_id == player);
            let local_choice = parked_choice
                || batch.events.iter().any(|event| match event.ev.as_ref() {
                    Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => {
                        choice.deciding_player_id == player
                    }
                    Some(rv1::ruled_event::Ev::TriggerNeedsTarget(choice)) => {
                        choice.controller_player_id == player
                    }
                    Some(rv1::ruled_event::Ev::TriggerOrderRequired(choice)) => {
                        choice.deciding_player_id == player
                    }
                    _ => false,
                });
            let combat_choice = self.state.priority_player_id() == player
                && super::combat::priority_locked_for_combat_declaration(&self.state);
            if (local_choice || combat_choice || self.state.cleanup_discard_player == Some(player))
                && self.automation_answer(player, &policy).is_none()
                && !policy.paused
            {
                policy.yielding = None;
                policy.paused = true;
                changed = true;
            }
            if changed {
                policy.revision = policy.revision.saturating_add(1);
                self.state.automation.insert(player, policy);
            }
        }
    }
}
