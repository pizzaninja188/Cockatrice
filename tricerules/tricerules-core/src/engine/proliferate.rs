//! Private action-time replacement state for Proliferate.
//!
//! The pending stack item remains owned by exactly one `ResolutionContinuation`. This transaction
//! carries only the lineage and receipts that must survive the continuation's recipient and
//! counter-replacement stages.

use crate::state::{
    CounterPlacementReceipt, ObjectId, OperationResults, PendingProliferateStage,
    PendingResolution, PendingResolutionPresentation, PlayerId, ReplacementSourcePresentation,
    ResolutionContinuation, Zone,
};
use std::collections::{BTreeSet, VecDeque};
use tricerules_card_model::primitives::StaticAbilityDef;
use tricerules_card_model::{AbilityDefinitionId, IdentifiedStaticAbility};
use tricerules_proto::ruled::v1 as rv1;

use super::events::{ev_log, finish_with_events, object_display_name};
use super::{EngineError, GameEngine, GameEvent};

pub(in crate::engine) enum InitialProliferateProgress {
    ContinuedInline,
    Parked,
}

enum ProliferateDriveProgress {
    ContinuedInline,
    Parked,
    Resumed(Box<rv1::RuledEventBatch>),
}

#[derive(Clone, Copy)]
struct ProliferateFaceIdentity {
    source_object_id: ObjectId,
    zone_change_generation: u64,
    face_index: usize,
}

/// One identified static-ability occurrence on one incarnation of a source permanent.
#[derive(serde::Serialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ProliferateReplacementIdentity {
    pub(crate) source_object_id: ObjectId,
    pub(crate) zone_change_generation: u64,
    /// Exact effective/copied face and ability path, not the displayed source name or a flattened
    /// index in the current active-static list.
    pub(crate) ability_occurrence: AbilityDefinitionId,
}

/// A Proliferate node inherits the replacements already applied on its lineage.
#[derive(serde::Serialize, Debug, Clone, Default)]
pub(crate) struct ProliferateNode {
    pub(crate) applied_replacements: BTreeSet<ProliferateReplacementIdentity>,
}

/// A checked prompt handle bound to the currently offered replacement identity.
#[derive(serde::Serialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProliferateReplacementApplication {
    pub(crate) application_id: u32,
    pub(crate) identity: ProliferateReplacementIdentity,
    pub(crate) presentation: ReplacementSourcePresentation,
}

/// One original Proliferate instruction and its replacement tree.
#[derive(serde::Serialize, Debug, Clone)]
pub(crate) struct PendingProliferateTransaction {
    /// The player who performed the original action. A delegate never changes this identity.
    pub(crate) actor: PlayerId,
    /// Nodes still waiting to become the next leaf, in deterministic replacement order.
    pub(crate) pending_nodes: VecDeque<ProliferateNode>,
    /// The lineage of the leaf currently being offered or resolved, when one has started.
    pub(crate) current_leaf: Option<ProliferateNode>,
    /// The exact source occurrences and public presentation bound to the current order prompt.
    pub(crate) offered_replacements: Vec<ProliferateReplacementApplication>,
    /// Number of completed leaves in this one original instruction.
    pub(crate) completed_leaves: u32,
    /// Actual committed positive counter changes across completed leaves so far.
    pub(crate) counter_placements: Vec<CounterPlacementReceipt>,
}

impl PendingProliferateTransaction {
    pub(crate) fn new(actor: PlayerId) -> Self {
        Self {
            actor,
            pending_nodes: VecDeque::from([ProliferateNode::default()]),
            current_leaf: None,
            offered_replacements: Vec::new(),
            completed_leaves: 0,
            counter_placements: Vec::new(),
        }
    }
}

impl GameEngine {
    /// Begin the shared replacement and recipient path for the original Proliferate action.
    pub(in crate::engine) fn begin_initial_proliferate_action(
        &mut self,
        stack: crate::state::ParkedStackResolution,
        mut transaction: PendingProliferateTransaction,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<InitialProliferateProgress, EngineError> {
        if transaction.pending_nodes.front().is_none() {
            return Err(EngineError::ProgramInvariant(
                "new Proliferate transaction has no root node",
            ));
        }
        transaction.current_leaf = transaction.pending_nodes.pop_front();
        match self.drive_proliferate_transaction(stack, transaction, events, false)? {
            ProliferateDriveProgress::ContinuedInline => {
                Ok(InitialProliferateProgress::ContinuedInline)
            }
            ProliferateDriveProgress::Parked => Ok(InitialProliferateProgress::Parked),
            ProliferateDriveProgress::Resumed(_) => Err(EngineError::ProgramInvariant(
                "initial Proliferate action unexpectedly resumed its parent stack",
            )),
        }
    }

    /// Advance the replacement tree until a player choice is required or every leaf is complete.
    fn drive_proliferate_transaction(
        &mut self,
        stack: crate::state::ParkedStackResolution,
        mut transaction: PendingProliferateTransaction,
        events: &mut Vec<rv1::RuledEvent>,
        resume_parent: bool,
    ) -> Result<ProliferateDriveProgress, EngineError> {
        loop {
            if transaction.current_leaf.is_none() {
                transaction.current_leaf = transaction.pending_nodes.pop_front();
            }
            let Some(node) = transaction.current_leaf.as_ref() else {
                if !resume_parent {
                    return Ok(ProliferateDriveProgress::ContinuedInline);
                }
                let batch = self.finish_proliferate_transaction(
                    stack,
                    transaction,
                    std::mem::take(events),
                )?;
                return Ok(ProliferateDriveProgress::Resumed(Box::new(batch)));
            };

            let applicable = self.double_controller_proliferation_replacements(
                transaction.actor,
                &node.applied_replacements,
            );
            match applicable.len() {
                0 => {
                    if self.begin_proliferate_leaf(&stack, transaction.clone(), events)? {
                        return Ok(ProliferateDriveProgress::Parked);
                    }
                    Self::mark_proliferate_leaf_complete(&mut transaction, &[])?;
                }
                1 => Self::split_current_proliferate_node(
                    &mut transaction,
                    applicable.into_iter().next().expect("one replacement").0,
                )?,
                _ => {
                    let applications =
                        self.make_proliferate_replacement_applications(applicable)?;
                    self.park_proliferate_replacement_order(
                        stack,
                        transaction,
                        applications,
                        events,
                    )?;
                    return Ok(ProliferateDriveProgress::Parked);
                }
            }
        }
    }

    /// Create the next leaf's existing mixed recipient choice. Returns `true` when a prompt was
    /// opened; an empty recipient set completes the leaf synchronously and returns `false`.
    fn begin_proliferate_leaf(
        &mut self,
        stack: &crate::state::ParkedStackResolution,
        transaction: PendingProliferateTransaction,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let actor = transaction.actor;
        let mut candidate_objects = self
            .state
            .objects
            .values()
            .filter(|object| {
                object.zone == Zone::Battlefield
                    && object.phase_status.is_none()
                    && object.counters.values().any(|count| *count > 0)
            })
            .map(|object| object.id)
            .collect::<Vec<_>>();
        candidate_objects.sort_unstable();
        let candidate_player_ids = self
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost && player.counters.values().any(|count| *count > 0))
            .map(|player| player.id)
            .collect::<Vec<_>>();

        if candidate_objects.is_empty() && candidate_player_ids.is_empty() {
            let first_unreplaced_leaf = transaction.completed_leaves == 0
                && transaction
                    .current_leaf
                    .as_ref()
                    .is_some_and(|leaf| leaf.applied_replacements.is_empty());
            if first_unreplaced_leaf {
                self.fire_triggers(&[GameEvent::Proliferated { player: actor }], events);
                events.push(ev_log(format!("P{actor} proliferates.")));
            } else {
                events.push(ev_log(format!("P{actor} proliferates.")));
                self.fire_triggers(&[GameEvent::Proliferated { player: actor }], events);
            }
            return Ok(false);
        }

        let candidate_generations = candidate_objects
            .iter()
            .map(|&object_id| {
                (
                    object_id,
                    self.state
                        .zone_change_generation
                        .get(&object_id)
                        .copied()
                        .unwrap_or(0),
                )
            })
            .collect::<Vec<_>>();
        let prompt = "Choose any number of permanents and/or players with counters.".to_string();
        let max = candidate_objects
            .len()
            .checked_add(candidate_player_ids.len())
            .and_then(|count| u32::try_from(count).ok())
            .ok_or(EngineError::Illegal("too many Proliferate recipients"))?;
        let (deciding_player, stage) = if self
            .state
            .player_idx(actor)
            .is_none_or(|index| self.state.players[index].has_lost)
        {
            let candidates =
                super::resolution::resolution_choice_delegate_candidates(&self.state, actor, actor);
            let selector =
                super::resolution::proliferate::prolif_delegate_selector(&self.state, actor)
                    .ok_or(EngineError::Illegal("no live Proliferate choice delegate"))?;
            let slots = candidates.into_iter().map(Some).collect::<Vec<_>>();
            (
                selector,
                PendingProliferateStage::ChoosingDelegate { candidates: slots },
            )
        } else {
            (actor, PendingProliferateStage::SelectingRecipients)
        };
        let pending_prompt = match &stage {
            PendingProliferateStage::ChoosingDelegate { .. } => format!(
                "P{deciding_player}: choose a player to make P{actor}'s Proliferate choice."
            ),
            PendingProliferateStage::SelectingRecipients => prompt.clone(),
        };
        let continuation = ResolutionContinuation::Proliferate {
            stack: stack.clone(),
            candidate_generations,
            candidate_player_ids: candidate_player_ids.clone(),
            stage,
            transaction,
        };
        let choosing_delegate = matches!(
            &continuation,
            ResolutionContinuation::Proliferate {
                stage: PendingProliferateStage::ChoosingDelegate { .. },
                ..
            }
        );
        self.state.pending_resolution = Some(PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,
            deciding_player,
            presentation: PendingResolutionPresentation {
                source_object_id: stack.item.id,
                candidates: if choosing_delegate {
                    Vec::new()
                } else {
                    candidate_objects.clone()
                },
                min: u32::from(choosing_delegate),
                max: if choosing_delegate { 1 } else { max },
                ordered: false,
                prompt: pending_prompt.clone(),
                choice_kind: if choosing_delegate {
                    rv1::ChoiceKind::ResolutionBranch
                } else {
                    rv1::ChoiceKind::Proliferate
                },
                unique_names: false,
            },
            continuation,
        });
        if stack.program.is_some() || choosing_delegate {
            events.push(
                self.proliferate_choice_event()
                    .ok_or(EngineError::ProgramInvariant(
                        "Proliferate prompt does not match its continuation",
                    ))?,
            );
        } else {
            events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                    rv1::ResolutionChoiceRequired {
                        resolution_prompt_id: 0,
                        variable_mana_contribution: false,
                        deciding_player_id: actor,
                        source_object_id: stack.item.id,
                        prompt_text: prompt,
                        choice_kind: rv1::ChoiceKind::Proliferate as i32,
                        candidate_object_ids: candidate_objects.clone(),
                        candidate_player_ids,
                        candidate_names: candidate_objects
                            .iter()
                            .map(|object_id| {
                                object_display_name(&self.state, self.registry, *object_id)
                            })
                            .collect(),
                        min: 0,
                        max,
                        ..Default::default()
                    },
                )),
            });
        }
        events.push(ev_log(pending_prompt));
        Ok(true)
    }

    fn mark_proliferate_leaf_complete(
        transaction: &mut PendingProliferateTransaction,
        receipts: &[CounterPlacementReceipt],
    ) -> Result<(), EngineError> {
        if transaction.current_leaf.take().is_none() {
            return Err(EngineError::ProgramInvariant(
                "completed Proliferate leaf has no current lineage",
            ));
        }
        transaction.completed_leaves =
            transaction
                .completed_leaves
                .checked_add(1)
                .ok_or(EngineError::ProgramInvariant(
                    "Proliferate completed leaf count overflowed",
                ))?;
        transaction.counter_placements.extend_from_slice(receipts);
        Ok(())
    }

    pub(in crate::engine) fn finish_proliferate_transaction(
        &mut self,
        stack: crate::state::ParkedStackResolution,
        transaction: PendingProliferateTransaction,
        events: Vec<rv1::RuledEvent>,
    ) -> Result<rv1::RuledEventBatch, EngineError> {
        if stack.program.is_some() {
            self.complete_parked_stack_with_previous(
                stack,
                OperationResults::default().with_counter_placements(transaction.counter_placements),
                events,
            )
        } else {
            let mut previous_result = stack.previous_result.clone();
            previous_result.record_counter_placements(transaction.counter_placements);
            self.complete_parked_resolution_with_previous(
                stack.item,
                stack.resume_effect_index,
                previous_result,
                events,
            )
        }
    }

    pub(in crate::engine) fn finish_proliferate_counter_leaf(
        &mut self,
        stack: crate::state::ParkedStackResolution,
        mut transaction: PendingProliferateTransaction,
        receipts: Vec<CounterPlacementReceipt>,
        mut events: Vec<rv1::RuledEvent>,
    ) -> Result<rv1::RuledEventBatch, EngineError> {
        if transaction.current_leaf.is_none() {
            transaction.current_leaf = transaction.pending_nodes.pop_front();
        }
        Self::mark_proliferate_leaf_complete(&mut transaction, &receipts)?;
        match self.drive_proliferate_transaction(stack, transaction, &mut events, true)? {
            ProliferateDriveProgress::Resumed(batch) => Ok(*batch),
            ProliferateDriveProgress::Parked | ProliferateDriveProgress::ContinuedInline => {
                Ok(finish_with_events(self, events))
            }
        }
    }

    pub(in crate::engine) fn finish_proliferate_replacement_choice(
        &mut self,
        pending: PendingResolution,
        answer: &rv1::SubmitResolutionChoice,
        decision: rv1::ResolutionChoiceDecision,
    ) -> Result<rv1::RuledEventBatch, EngineError> {
        let (stack, transaction) = match &pending.continuation {
            ResolutionContinuation::ProliferateReplacement { stack, transaction } => {
                (stack.clone(), transaction.clone())
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("not a Proliferate replacement choice"));
            }
        };
        let payload_is_valid = decision == rv1::ResolutionChoiceDecision::Unspecified
            && answer.chosen_object_ids.len() == 1
            && answer.chosen_player_ids.is_empty()
            && answer.selected_branch_index == 0
            && answer.cast_spell.is_none()
            && answer.spell_cast_announcement.is_none()
            && answer.chosen_combat_defender.is_none()
            && answer.payment.is_none()
            && answer.restricted_mana.is_empty();
        let selected = payload_is_valid.then(|| {
            transaction
                .offered_replacements
                .iter()
                .find(|application| application.application_id == answer.chosen_object_ids[0])
                .cloned()
        });
        let Some(Some(selected)) = selected else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "choose one published Proliferate replacement",
            ));
        };
        if self.state.pending_replacement_event.is_some() {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "Proliferate replacement order conflicts with another replacement event",
            ));
        }
        let Some(current_leaf) = transaction.current_leaf.as_ref() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::ProgramInvariant(
                "Proliferate replacement prompt lost its current node",
            ));
        };
        let current = self.double_controller_proliferation_replacements(
            transaction.actor,
            &current_leaf.applied_replacements,
        );
        let exact_offer_is_current = current.len() == transaction.offered_replacements.len()
            && current
                .iter()
                .zip(&transaction.offered_replacements)
                .all(|((identity, _), application)| *identity == application.identity);
        let expected_chooser = super::resolution::proliferate::prolif_delegate_selector(
            &self.state,
            transaction.actor,
        );
        if !exact_offer_is_current || expected_chooser != Some(pending.deciding_player) {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "Proliferate replacement offer or chooser is stale",
            ));
        }

        let mut transaction = transaction;
        transaction.offered_replacements.clear();
        if let Err(error) =
            Self::split_current_proliferate_node(&mut transaction, selected.identity)
        {
            self.state.pending_resolution = Some(pending);
            return Err(error);
        }
        let mut events = Vec::new();
        match self.drive_proliferate_transaction(stack, transaction, &mut events, true)? {
            ProliferateDriveProgress::Resumed(batch) => Ok(*batch),
            ProliferateDriveProgress::Parked | ProliferateDriveProgress::ContinuedInline => {
                Ok(finish_with_events(self, events))
            }
        }
    }

    /// Reconcile an outer replacement-order prompt after a player leaves. Preserve the original
    /// action actor and parked parent, then rebuild the current source offer after concession has
    /// removed departed-owned permanents. A zero or single live offer returns to normal action
    /// processing; multiple live occurrences get a fresh prompt for the current delegate.
    pub(in crate::engine) fn refresh_proliferate_replacement_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.take() else {
            return Ok(());
        };
        let (stack, mut transaction) = match &pending.continuation {
            ResolutionContinuation::ProliferateReplacement { stack, transaction } => {
                (stack.clone(), transaction.clone())
            }
            _ => {
                self.state.pending_resolution = Some(pending);
                return Ok(());
            }
        };
        let Some(current_leaf) = transaction.current_leaf.as_ref() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::ProgramInvariant(
                "Proliferate replacement prompt lost its current node",
            ));
        };
        let current = self.double_controller_proliferation_replacements(
            transaction.actor,
            &current_leaf.applied_replacements,
        );
        let offer_is_current = current.len() == transaction.offered_replacements.len()
            && current
                .iter()
                .zip(&transaction.offered_replacements)
                .all(|((identity, _), application)| *identity == application.identity);
        let current_chooser = super::resolution::proliferate::prolif_delegate_selector(
            &self.state,
            transaction.actor,
        );
        if offer_is_current && current_chooser == Some(pending.deciding_player) {
            self.state.pending_resolution = Some(pending);
            return Ok(());
        }

        let actor = transaction.actor;
        transaction.offered_replacements.clear();
        if current.len() >= 2 {
            let applications = self.make_proliferate_replacement_applications(current)?;
            events.push(ev_log(format!(
                "P{actor}'s Proliferate replacement order offer was refreshed."
            )));
            self.park_proliferate_replacement_order(stack, transaction, applications, events)?;
            return Ok(());
        }

        match self.drive_proliferate_transaction(stack, transaction, events, true)? {
            ProliferateDriveProgress::Resumed(batch) => events.extend(batch.events),
            ProliferateDriveProgress::Parked | ProliferateDriveProgress::ContinuedInline => {}
        }
        Ok(())
    }

    /// Enumerate active, available Tekuthal-style replacement occurrences for this action actor.
    /// The identified ability path is rebuilt from the current copied face so two effects on one
    /// permanent remain separate occurrences and a face/copy change cannot impersonate an old
    /// application.
    pub(in crate::engine) fn double_controller_proliferation_replacements(
        &self,
        actor: PlayerId,
        already_applied: &BTreeSet<ProliferateReplacementIdentity>,
    ) -> Vec<(
        ProliferateReplacementIdentity,
        ReplacementSourcePresentation,
    )> {
        let mut sources = self.state.objects.keys().copied().collect::<Vec<_>>();
        sources.sort_unstable();
        let mut replacements = Vec::new();

        for source in sources {
            if !super::characteristics::printed_static_source_is_available(
                &self.state,
                self.registry,
                source,
            ) {
                continue;
            }
            let Some(characteristics) = self.characteristics(source) else {
                continue;
            };
            if characteristics.controller != actor {
                continue;
            }
            let active_ability_ids = self
                .active_static_abilities(source)
                .into_iter()
                .map(|ability| ability.ability_id)
                .collect::<BTreeSet<_>>();
            if active_ability_ids.is_empty() {
                continue;
            }
            let generation = self
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0);
            let presentation = self.replacement_source_presentation(source);

            if let Some(faces) = self.room_faces(source) {
                let unlocked = self
                    .state
                    .room_states
                    .get(&source)
                    .copied()
                    .unwrap_or_default()
                    .unlocked_indices();
                for door in unlocked {
                    let Some(face) = faces.get(door) else {
                        continue;
                    };
                    self.append_double_controller_proliferation_occurrences(
                        ProliferateFaceIdentity {
                            source_object_id: source,
                            zone_change_generation: generation,
                            face_index: door,
                        },
                        &face.static_abilities,
                        &active_ability_ids,
                        &presentation,
                        already_applied,
                        &mut replacements,
                    );
                }
            } else if let (Some(face), Some(object)) =
                (self.effective_face(source), self.state.objects.get(&source))
            {
                let level = self.state.class_level(source);
                let mut abilities = face.static_abilities.clone();
                abilities.extend(
                    face.class_level_bars
                        .iter()
                        .filter(|bar| level >= bar.level)
                        .flat_map(|bar| bar.static_abilities.iter().cloned()),
                );
                self.append_double_controller_proliferation_occurrences(
                    ProliferateFaceIdentity {
                        source_object_id: source,
                        zone_change_generation: generation,
                        face_index: object.face_up_index,
                    },
                    &abilities,
                    &active_ability_ids,
                    &presentation,
                    already_applied,
                    &mut replacements,
                );
            }
        }

        replacements.sort_by(|left, right| left.0.cmp(&right.0));
        replacements.dedup_by(|left, right| left.0 == right.0);
        replacements
    }

    fn append_double_controller_proliferation_occurrences(
        &self,
        face: ProliferateFaceIdentity,
        abilities: &[IdentifiedStaticAbility],
        active_ability_ids: &BTreeSet<tricerules_card_model::AbilityId>,
        presentation: &ReplacementSourcePresentation,
        already_applied: &BTreeSet<ProliferateReplacementIdentity>,
        replacements: &mut Vec<(
            ProliferateReplacementIdentity,
            ReplacementSourcePresentation,
        )>,
    ) {
        for ability in abilities.iter().filter(|ability| {
            active_ability_ids.contains(&ability.ability_id)
                && matches!(
                    &ability.definition,
                    StaticAbilityDef::DoubleControllerProliferations
                )
        }) {
            let identity = ProliferateReplacementIdentity {
                source_object_id: face.source_object_id,
                zone_change_generation: face.zone_change_generation,
                ability_occurrence: self.ability_definition(
                    face.source_object_id,
                    face.face_index,
                    vec![ability.ability_id.clone()],
                ),
            };
            if !already_applied.contains(&identity) {
                replacements.push((identity, presentation.clone()));
            }
        }
    }

    /// Mark the current node's selected source and prepend two independently owned child
    /// lineages. Depth-first processing completes all nested replacement children before the
    /// parent's other child is advanced.
    pub(in crate::engine) fn split_current_proliferate_node(
        transaction: &mut PendingProliferateTransaction,
        identity: ProliferateReplacementIdentity,
    ) -> Result<(), EngineError> {
        let Some(mut current) = transaction.current_leaf.take() else {
            return Err(EngineError::ProgramInvariant(
                "Proliferate replacement has no current node",
            ));
        };
        if !current.applied_replacements.insert(identity) {
            transaction.current_leaf = Some(current);
            return Err(EngineError::Illegal(
                "Proliferate replacement was already applied on this lineage",
            ));
        }
        let first_child = ProliferateNode {
            applied_replacements: current.applied_replacements.clone(),
        };
        let second_child = ProliferateNode {
            applied_replacements: current.applied_replacements,
        };
        transaction.pending_nodes.push_front(second_child);
        transaction.pending_nodes.push_front(first_child);
        Ok(())
    }

    /// Bind replacement handles to the exact current offer with checked, non-wrapping IDs.
    pub(in crate::engine) fn make_proliferate_replacement_applications(
        &mut self,
        replacements: Vec<(
            ProliferateReplacementIdentity,
            ReplacementSourcePresentation,
        )>,
    ) -> Result<Vec<ProliferateReplacementApplication>, EngineError> {
        let count = u32::try_from(replacements.len())
            .map_err(|_| EngineError::Illegal("too many Proliferate replacement applications"))?;
        let first_id = self.state.next_replacement_application_id;
        let next_id = first_id.checked_add(count).ok_or(EngineError::Illegal(
            "Proliferate replacement application id exhausted",
        ))?;
        let mut applications = Vec::with_capacity(replacements.len());
        for (offset, (identity, presentation)) in replacements.into_iter().enumerate() {
            let offset = u32::try_from(offset).map_err(|_| {
                EngineError::Illegal("too many Proliferate replacement applications")
            })?;
            applications.push(ProliferateReplacementApplication {
                application_id: first_id.checked_add(offset).ok_or(EngineError::Illegal(
                    "Proliferate replacement application id exhausted",
                ))?,
                identity,
                presentation,
            });
        }
        self.state.next_replacement_application_id = next_id;
        Ok(applications)
    }

    /// Project an action-time replacement-order offer through the existing opaque replacement
    /// option protocol. The continuation owns the offer while the shared counter replacement
    /// slot stays available for a later recipient batch.
    pub(in crate::engine) fn park_proliferate_replacement_order(
        &mut self,
        stack: crate::state::ParkedStackResolution,
        mut transaction: PendingProliferateTransaction,
        applications: Vec<ProliferateReplacementApplication>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        if applications.len() < 2 {
            return Err(EngineError::ProgramInvariant(
                "Proliferate order prompt requires multiple applicable replacements",
            ));
        }
        let chooser = super::resolution::proliferate::prolif_delegate_selector(
            &self.state,
            transaction.actor,
        )
        .ok_or(EngineError::Illegal(
            "no live player can choose the Proliferate replacement order",
        ))?;
        transaction.offered_replacements = applications;
        let candidates = transaction
            .offered_replacements
            .iter()
            .map(|application| application.presentation.object_id)
            .collect::<Vec<_>>();
        self.state.pending_resolution = Some(crate::state::PendingResolution {
            resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
            resolution_prompt_semantics: None,
            deciding_player: chooser,
            presentation: crate::state::PendingResolutionPresentation {
                source_object_id: stack.item.id,
                candidates,
                min: 1,
                max: 1,
                ordered: false,
                prompt: "Choose the next replacement effect for Proliferate.".to_string(),
                choice_kind: rv1::ChoiceKind::ReplacementEffect,
                unique_names: false,
            },
            continuation: crate::state::ResolutionContinuation::ProliferateReplacement {
                stack,
                transaction,
            },
        });
        events.push(self.proliferate_replacement_choice_event().ok_or(
            EngineError::ProgramInvariant(
                "Proliferate replacement prompt does not match its continuation",
            ),
        )?);
        Ok(())
    }

    pub(in crate::engine) fn proliferate_replacement_choice_event(
        &self,
    ) -> Option<rv1::RuledEvent> {
        let pending = self.state.pending_resolution.as_ref()?;
        let crate::state::ResolutionContinuation::ProliferateReplacement { transaction, .. } =
            &pending.continuation
        else {
            return None;
        };
        Some(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    variable_mana_contribution: false,
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: pending.presentation.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ReplacementEffect as i32,
                    candidate_object_ids: pending.presentation.candidates.clone(),
                    candidate_names: transaction
                        .offered_replacements
                        .iter()
                        .map(|application| application.presentation.card_name.clone())
                        .collect(),
                    candidate_selectable: vec![true; transaction.offered_replacements.len()],
                    replacement_options: transaction
                        .offered_replacements
                        .iter()
                        .map(|application| {
                            application.presentation.option(
                                application.application_id,
                                "Proliferate twice instead.".to_string(),
                            )
                        })
                        .collect(),
                    min: 1,
                    max: 1,
                    ..Default::default()
                },
            )),
        })
    }
}

#[cfg(test)]
mod tests;
