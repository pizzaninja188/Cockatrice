//! Runtime for the finite, registry-compiled spell-resolution program subset.
//!
//! The registry owns immutable instructions. Parked state contains only the active nested block
//! paths, cursors, typed values, and the result slots for one operation whose domain transaction
//! has not completed yet.

use super::*;
use crate::engine::presentation::{
    stack_child_presentation_ref, PresentationPath, StackPresentationSource,
};
use crate::state::{
    DiscardCardTypeKnowledge, ProgramArm, ProgramDiscardResult, ProgramFrame, ProgramPathStep,
    ProgramRunState, ProgramValue,
};
use tricerules_card_model::primitives::{
    CompiledBlock, CompiledChooseBranch, CompiledChooseRequirement, CompiledInstructionKind,
    CompiledProgram, CompiledProgramCondition, CompiledProgramResultCondition,
    ProgramRequiredInput, ResolutionCost, ResultSlotId, ResultType,
};

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgramFailpoint {
    DomainCommitted,
    SlotPublished,
    OfferPublished,
}

#[cfg(test)]
thread_local! {
    static PROGRAM_FAILPOINT: std::cell::Cell<Option<ProgramFailpoint>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
struct ProgramFailpointGuard {
    previous: Option<ProgramFailpoint>,
}

#[cfg(test)]
impl ProgramFailpointGuard {
    fn arm(point: ProgramFailpoint) -> Self {
        let previous = PROGRAM_FAILPOINT.with(|active| {
            let previous = active.get();
            assert!(
                previous.is_none(),
                "program failpoint already armed on this thread"
            );
            active.set(Some(point));
            previous
        });
        Self { previous }
    }
}

#[cfg(test)]
impl Drop for ProgramFailpointGuard {
    fn drop(&mut self) {
        PROGRAM_FAILPOINT.with(|active| active.set(self.previous));
    }
}

#[cfg(test)]
fn trip_program_failpoint(point: ProgramFailpoint) -> Result<(), EngineError> {
    let armed = PROGRAM_FAILPOINT.with(|active| {
        if active.get() == Some(point) {
            active.set(None);
            true
        } else {
            false
        }
    });
    if armed {
        let reason = match point {
            ProgramFailpoint::DomainCommitted => "after domain commit",
            ProgramFailpoint::SlotPublished => "after result-slot publication",
            ProgramFailpoint::OfferPublished => "after choice-offer publication",
        };
        Err(EngineError::ProgramInvariant(reason))
    } else {
        Ok(())
    }
}

impl GameEngine {
    /// Select the exact immutable program units for this stack occurrence. A compiled modal
    /// container owns every sibling mode, so any missing sibling is an invariant failure rather
    /// than a reason to mix program execution with the legacy flattened effect list.
    pub(in crate::engine) fn compiled_resolution_program_owners(
        &self,
        top: &StackItem,
    ) -> Result<Option<Vec<tricerules_card_model::ProgramOwnerId>>, EngineError> {
        use tricerules_card_model::ProgramOwnerId;

        let is_ability = top.ability_text.is_some();
        let captured_authored_program_body = if top.is_triggered {
            top.triggered_ability.as_ref().is_some_and(|ability| {
                ability.program.is_some()
                    || ability
                        .modal
                        .as_ref()
                        .is_some_and(|modal| modal.modes.iter().any(|mode| mode.program.is_some()))
            })
        } else {
            top.activated_ability
                .as_ref()
                .is_some_and(|ability| ability.program.is_some())
        };
        let base_owner = if is_ability {
            let Some(owner) = top.ability_program_owner() else {
                if captured_authored_program_body {
                    return Err(EngineError::ProgramInvariant(
                        "captured authored ability program has no exact owner occurrence",
                    ));
                }
                // Intrinsic mana, keyword-granted, and resolving-grant abilities retain their
                // legacy behavior because they have no authored definition owner.
                return Ok(None);
            };
            owner
        } else {
            let Some(face) = self
                .registry
                .get(&top.card_id)
                .and_then(|definition| definition.face(top.face_index))
            else {
                return Ok(None);
            };
            ProgramOwnerId::face(top.card_id.clone(), face.face_id.clone())
        };

        let owner_for_mode = |mode_id: &tricerules_card_model::ModeId| match &base_owner {
            ProgramOwnerId::Face(face) => ProgramOwnerId::FaceMode {
                face: face.clone(),
                mode_id: mode_id.clone(),
            },
            ProgramOwnerId::Ability(ability) => ProgramOwnerId::AbilityMode {
                ability: ability.clone(),
                mode_id: mode_id.clone(),
            },
            ProgramOwnerId::FaceMode { .. } | ProgramOwnerId::AbilityMode { .. } => {
                unreachable!("stack abilities carry their unsuffixed authored owner")
            }
        };
        let selected_mode_has_compiled_owner = || {
            top.chosen_modes.iter().any(|chosen| {
                self.registry
                    .compiled_program_for_owner(&owner_for_mode(&chosen.mode_id))
                    .is_some()
            })
        };

        // Authored abilities resolve from the exact definition captured when the occurrence was
        // announced. A currently indexed face ability is display/source data, not a recovery path
        // for a compiled body that was lost from the stack item.
        if is_ability {
            let has_captured_body = if top.is_triggered {
                top.triggered_ability.is_some()
            } else {
                top.activated_ability.is_some()
            };
            if !has_captured_body
                && (self
                    .registry
                    .compiled_program_for_owner(&base_owner)
                    .is_some()
                    || selected_mode_has_compiled_owner())
            {
                return Err(EngineError::ProgramInvariant(
                    "compiled ability program is missing its captured definition",
                ));
            }
        }

        if !top.chosen_modes.is_empty() {
            let modal = if top.is_triggered {
                top.triggered_ability
                    .as_ref()
                    .and_then(|ability| ability.modal.as_ref())
            } else if is_ability {
                None
            } else {
                self.registry
                    .get(&top.card_id)
                    .and_then(|definition| definition.face(top.face_index))
                    .and_then(|face| face.modal_spell.as_ref())
            };
            if top.is_triggered && modal.is_none() && selected_mode_has_compiled_owner() {
                return Err(EngineError::ProgramInvariant(
                    "compiled modal ability is missing its captured mode definition",
                ));
            }
            if let Some(modal) = modal {
                let sibling_owners = modal
                    .modes
                    .iter()
                    .map(|mode| owner_for_mode(&mode.mode_id))
                    .collect::<Vec<_>>();
                let any_sibling_compiled = sibling_owners
                    .iter()
                    .any(|owner| self.registry.compiled_program_for_owner(owner).is_some());
                if any_sibling_compiled {
                    if sibling_owners
                        .iter()
                        .any(|owner| self.registry.compiled_program_for_owner(owner).is_none())
                    {
                        return Err(EngineError::ProgramInvariant(
                            "compiled modal owner is missing a sibling mode program",
                        ));
                    }
                    let selected_owners = top
                        .chosen_modes
                        .iter()
                        .map(|chosen| {
                            if modal.mode_by_id(&chosen.mode_id).is_none() {
                                return Err(EngineError::ProgramInvariant(
                                    "stack item selected an unknown modal program owner",
                                ));
                            }
                            let owner = owner_for_mode(&chosen.mode_id);
                            self.registry
                                .compiled_program_for_owner(&owner)
                                .map(|_| owner)
                                .ok_or(EngineError::ProgramInvariant(
                                    "selected modal sibling program was not compiled",
                                ))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(Some(selected_owners));
                }
            }
        }

        if self
            .registry
            .compiled_program_for_owner(&base_owner)
            .is_some()
        {
            return Ok(Some(vec![base_owner]));
        }
        if !is_ability
            && self
                .registry
                .get(&top.card_id)
                .and_then(|definition| definition.face(top.face_index))
                .is_some_and(|face| face.spell_program.is_some())
        {
            return Err(EngineError::ProgramInvariant(
                "authored spell program owner was not compiled",
            ));
        }
        if captured_authored_program_body {
            return Err(EngineError::ProgramInvariant(
                "captured authored ability program was not compiled for its exact owner",
            ));
        }
        Ok(None)
    }

    pub(in crate::engine) fn current_program_perform(
        &self,
        program_state: &ProgramRunState,
    ) -> Result<SpellEffectKind, EngineError> {
        let compiled = self
            .registry
            .compiled_program_for_owner(&program_state.owner)
            .ok_or(EngineError::ProgramInvariant(
                "compiled resolution program disappeared",
            ))?;
        let frame = program_state
            .frames
            .last()
            .ok_or(EngineError::ProgramInvariant(
                "resolution program lost its active frame",
            ))?;
        let instruction_index =
            frame
                .next_instruction
                .checked_sub(1)
                .ok_or(EngineError::ProgramInvariant(
                    "resolution program has no in-flight instruction",
                ))?;
        let block = block_at_path(compiled, &frame.path)?;
        let instruction =
            block
                .steps
                .get(instruction_index as usize)
                .ok_or(EngineError::ProgramInvariant(
                    "resolution program in-flight cursor is invalid",
                ))?;
        let CompiledInstructionKind::Perform(effect) = &instruction.kind else {
            return Err(EngineError::ProgramInvariant(
                "resolution program choice was not a leaf operation",
            ));
        };
        Ok(effect.as_ref().clone())
    }

    pub(in crate::engine) fn run_program_with_previous(
        &mut self,
        top: &StackItem,
        program_state: ProgramRunState,
        previous_result: EffectResult,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<ResolutionProgress, EngineError> {
        self.run_program_with_previous_inner(top, program_state, previous_result, None, events)
            .map_err(|error| match error {
                // Once a compiled, bounded program starts, an Illegal result can only mean its
                // authored contract or committed internal transaction became inconsistent.
                // Public actor, generation, and offered-choice validation runs before this point.
                EngineError::Illegal(reason) => EngineError::ProgramInvariant(reason),
                other => other,
            })
    }

    pub(in crate::engine) fn resume_program_with_completion(
        &mut self,
        top: &StackItem,
        program_state: ProgramRunState,
        previous_result: EffectResult,
        operation_result: Option<EffectResult>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<ResolutionProgress, EngineError> {
        self.run_program_with_previous_inner(
            top,
            program_state,
            previous_result,
            operation_result,
            events,
        )
        .map_err(|error| match error {
            EngineError::Illegal(reason) => EngineError::ProgramInvariant(reason),
            other => other,
        })
    }

    pub(in crate::engine) fn begin_program_run(
        &self,
        top: &StackItem,
        mut owners: Vec<tricerules_card_model::ProgramOwnerId>,
    ) -> Result<ProgramRunState, EngineError> {
        let owner = owners
            .first()
            .cloned()
            .ok_or(EngineError::ProgramInvariant(
                "resolution program has no selected owner",
            ))?;
        let compiled = self.registry.compiled_program_for_owner(&owner).ok_or(
            EngineError::ProgramInvariant("selected resolution program owner was not compiled"),
        )?;
        validate_program_unit_inputs(top, compiled)?;
        owners.remove(0);
        Ok(ProgramRunState {
            owner,
            pending_owners: owners,
            frames: vec![ProgramFrame {
                path: Vec::new(),
                next_instruction: 0,
                yield_to: Vec::new(),
            }],
            slots: vec![None; compiled.slots.len()],
            in_flight: None,
        })
    }

    fn run_program_with_previous_inner(
        &mut self,
        top: &StackItem,
        mut program_state: ProgramRunState,
        previous_result: EffectResult,
        operation_result: Option<EffectResult>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<ResolutionProgress, EngineError> {
        if self.state.is_terminal() {
            return Ok(ResolutionProgress::GameEnded);
        }
        let expected_owners =
            self.compiled_resolution_program_owners(top)?
                .ok_or(EngineError::ProgramInvariant(
                    "parked resolution program lost its exact authored owner",
                ))?;
        // The active owner advances through selected modal units as each one finishes. The
        // pending list is the immutable suffix after that active unit; derive its position from
        // the suffix length so repeated selections of the same mode remain unambiguous.
        let active_owner_index = expected_owners
            .len()
            .checked_sub(program_state.pending_owners.len().saturating_add(1));
        if active_owner_index
            .and_then(|index| expected_owners.get(index).map(|owner| (index, owner)))
            .is_none_or(|(index, owner)| {
                owner != &program_state.owner
                    || expected_owners.get(index + 1..)
                        != Some(program_state.pending_owners.as_slice())
            })
        {
            return Err(EngineError::ProgramInvariant(
                "resolution program owner sequence changed",
            ));
        }

        // Copy the shared registry reference before entering the mutable execution loop. The
        // compiled tree remains borrowed from this immutable process-wide registry throughout;
        // only the currently executing leaf operation is cloned.
        let registry = self.registry;
        let active_compiled = registry
            .compiled_program_for_owner(&program_state.owner)
            .ok_or(EngineError::ProgramInvariant(
                "compiled resolution program disappeared",
            ))?;
        if active_compiled.owner != program_state.owner
            || active_compiled.context != program_state.owner.effect_context()
            || active_compiled.target_projection != program_state.owner.target_projection()
        {
            return Err(EngineError::ProgramInvariant(
                "resolution program context or target projection changed",
            ));
        }
        validate_program_unit_inputs(top, active_compiled)?;
        if program_state.slots.len() != active_compiled.slots.len()
            || program_state.frames.is_empty()
        {
            return Err(EngineError::ProgramInvariant(
                "invalid resolution program cursor",
            ));
        }

        if let Some(outputs) = program_state.in_flight.take() {
            let operation_result =
                operation_result
                    .as_ref()
                    .ok_or(EngineError::ProgramInvariant(
                        "parked program operation completed without its result",
                    ))?;
            #[cfg(test)]
            trip_program_failpoint(ProgramFailpoint::DomainCommitted)?;
            capture_program_outputs(
                active_compiled,
                &mut program_state,
                &outputs,
                operation_result,
            )?;
            #[cfg(test)]
            if !outputs.is_empty() {
                trip_program_failpoint(ProgramFailpoint::SlotPublished)?;
            }
            if self.finish_program_instruction_boundary(
                top,
                &program_state,
                &previous_result,
                events,
            )? {
                return Ok(ResolutionProgress::Parked);
            }
        } else if operation_result.is_some() {
            return Err(EngineError::ProgramInvariant(
                "program operation result has no in-flight output",
            ));
        }

        loop {
            if self.state.is_terminal() {
                return Ok(ResolutionProgress::GameEnded);
            }
            let compiled = registry
                .compiled_program_for_owner(&program_state.owner)
                .ok_or(EngineError::ProgramInvariant(
                    "compiled resolution program disappeared",
                ))?;
            if compiled.owner != program_state.owner
                || compiled.context != program_state.owner.effect_context()
                || compiled.target_projection != program_state.owner.target_projection()
            {
                return Err(EngineError::ProgramInvariant(
                    "resolution program context or target projection changed",
                ));
            }
            validate_program_unit_inputs(top, compiled)?;
            if program_state.slots.len() != compiled.slots.len() || program_state.frames.is_empty()
            {
                return Err(EngineError::ProgramInvariant(
                    "invalid resolution program cursor",
                ));
            }
            let Some(frame_index) = program_state.frames.len().checked_sub(1) else {
                return Err(EngineError::ProgramInvariant(
                    "resolution program lost its root frame",
                ));
            };
            let path = program_state.frames[frame_index].path.clone();
            let block = block_at_path(compiled, &path)?;
            let instruction_index = program_state.frames[frame_index].next_instruction as usize;

            if instruction_index >= block.steps.len() {
                let finished = program_state.frames.pop().expect("frame index checked");
                if !program_state.frames.is_empty() {
                    let child = block_at_path(compiled, &finished.path)?;
                    if !finished.yield_to.is_empty() {
                        if child.yields.len() != finished.yield_to.len() {
                            return Err(EngineError::ProgramInvariant(
                                "resolution program branch yield arity changed",
                            ));
                        }
                        for (source, destination) in child.yields.iter().zip(&finished.yield_to) {
                            let value = program_state
                                .slots
                                .get(source.0 as usize)
                                .and_then(Option::as_ref)
                                .cloned()
                                .ok_or(EngineError::ProgramInvariant(
                                    "resolution program yielded an unavailable result",
                                ))?;
                            let slot = program_state.slots.get_mut(destination.0 as usize).ok_or(
                                EngineError::ProgramInvariant(
                                    "resolution program branch output is invalid",
                                ),
                            )?;
                            *slot = Some(value);
                        }
                    }
                    continue;
                }

                if let Some(next_owner) = program_state.pending_owners.first().cloned() {
                    let next = registry.compiled_program_for_owner(&next_owner).ok_or(
                        EngineError::ProgramInvariant("selected modal sibling program disappeared"),
                    )?;
                    validate_program_unit_inputs(top, next)?;
                    program_state.pending_owners.remove(0);
                    program_state.owner = next_owner;
                    program_state.frames = vec![ProgramFrame {
                        path: Vec::new(),
                        next_instruction: 0,
                        yield_to: Vec::new(),
                    }];
                    program_state.slots = vec![None; next.slots.len()];
                    program_state.in_flight = None;
                    continue;
                }

                let label = self.build_resolution_effects(top).1;
                self.finish_deferred_stack_exit(top, DeferredStackExit::Resolved, events)?;
                events.push(ev_log(format!("{label} resolves.")));
                seat_resolved_spell_last_in_graveyard(&mut self.state, top.id);
                return Ok(ResolutionProgress::Completed);
            }

            let instruction = &block.steps[instruction_index];
            match &instruction.kind {
                CompiledInstructionKind::If { condition, yes, no } => {
                    let condition = condition.clone();
                    let outputs = instruction.outputs.clone();
                    let parent_path = path;
                    let holds = match condition.as_ref() {
                        CompiledProgramCondition::Game(condition) => {
                            self.condition_holds(condition, ConditionContext::for_stack_item(top))
                        }
                        CompiledProgramCondition::Result(condition) => {
                            program_result_condition_holds(
                                &self.state,
                                top,
                                &program_state,
                                condition,
                            )?
                        }
                    };
                    let selected_arm = if holds {
                        ProgramArm::Yes
                    } else {
                        ProgramArm::No
                    };
                    let _ = (yes, no);
                    program_state.frames[frame_index].next_instruction += 1;
                    let mut child_path = parent_path;
                    child_path.push(ProgramPathStep {
                        instruction_index: instruction_index as u32,
                        arm: selected_arm,
                    });
                    program_state.frames.push(ProgramFrame {
                        path: child_path,
                        next_instruction: 0,
                        yield_to: outputs,
                    });
                }
                CompiledInstructionKind::Choose {
                    choice,
                    optional,
                    branches,
                    otherwise,
                    ..
                } => {
                    let occurrence = crate::state::ProgramChooseOccurrence {
                        parent_path: path.clone(),
                        instruction_index: instruction_index as u32,
                        choice_id: choice.clone(),
                    };
                    let choose_controller_live = self
                        .state
                        .player_idx(top.controller)
                        .is_some_and(|idx| !self.state.players[idx].has_lost);
                    let live_branches = branches
                        .iter()
                        .enumerate()
                        .filter(|(_, branch)| {
                            (choose_controller_live || branch.cost == ResolutionCost::None)
                                && self.program_choose_requirement_holds(top, &branch.requirement)
                                && self.program_choose_draw_is_selectable(
                                    top,
                                    &previous_result,
                                    branch,
                                )
                        })
                        .map(|(index, _)| index)
                        .collect::<Vec<_>>();
                    program_state.frames[frame_index].next_instruction += 1;
                    if live_branches.is_empty() {
                        let mut child_path = path;
                        child_path.push(ProgramPathStep {
                            instruction_index: instruction_index as u32,
                            arm: ProgramArm::ChooseOtherwise,
                        });
                        program_state.frames.push(ProgramFrame {
                            path: child_path,
                            next_instruction: 0,
                            yield_to: instruction.outputs.clone(),
                        });
                        let _ = otherwise;
                        continue;
                    }
                    let prompt = if *optional {
                        "Choose a resolution option, or decline.".to_string()
                    } else {
                        "Choose a resolution option.".to_string()
                    };
                    self.state.pending_resolution = Some(crate::state::PendingResolution {
                        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
                        resolution_prompt_semantics: None,
                        deciding_player: top.controller,
                        presentation: crate::state::PendingResolutionPresentation {
                            source_object_id: top.id,
                            candidates: Vec::new(),
                            min: u32::from(!*optional),
                            max: 1,
                            ordered: false,
                            prompt: prompt.clone(),
                            choice_kind: rv1::ChoiceKind::ResolutionBranch,
                            unique_names: false,
                        },
                        continuation: crate::state::ResolutionContinuation::ProgramChoose {
                            stack: ParkedStackResolution {
                                item: top.clone(),
                                resume_effect_index: None,
                                previous_result: previous_result.clone(),
                                program: Some(program_state),
                            },
                            choose: crate::state::PendingProgramChoose {
                                occurrence,
                                original_chooser: top.controller,
                                optional: *optional,
                                delegate_selected: false,
                                stage: crate::state::PendingProgramChooseStage::Selecting,
                            },
                        },
                    });
                    #[cfg(test)]
                    trip_program_failpoint(ProgramFailpoint::OfferPublished)?;
                    events.push(self.program_choose_choice_event().ok_or(
                        EngineError::ProgramInvariant("program Choose offer could not be rendered"),
                    )?);
                    events.push(ev_log(prompt));
                    return Ok(ResolutionProgress::Parked);
                }
                CompiledInstructionKind::Perform(effect) => {
                    let effect = effect.as_ref().clone();
                    let outputs = instruction.outputs.clone();
                    let input = instruction.input;
                    let target_context =
                        program_target_context(top, compiled, instruction.instruction_address)?;
                    program_state.frames[frame_index].next_instruction += 1;
                    program_state.in_flight = Some(outputs.clone());

                    let mut previous = EffectResult::default();
                    if let Some(input) = input {
                        match program_state
                            .slots
                            .get(input.0 as usize)
                            .and_then(Option::as_ref)
                            .ok_or(EngineError::ProgramInvariant(
                                "resolution program input is unavailable",
                            ))? {
                            ProgramValue::PermanentSelection(objects) => {
                                previous.produced_objects = objects.clone();
                            }
                            ProgramValue::HandSelection(objects) => {
                                previous.selected_hand_cards = objects.clone();
                            }
                            ProgramValue::CardResultCohort(cohort) => {
                                previous.cards = cohort.cards.clone();
                            }
                            value @ (ProgramValue::FoundLibraryCohort(_)
                            | ProgramValue::RevealedLibraryCohort(_)
                            | ProgramValue::CardPartition(_)) => {
                                previous.program_value = Some(value.clone());
                            }
                            _ => {
                                return Err(EngineError::ProgramInvariant(
                                    "resolution program input has the wrong type",
                                ))
                            }
                        }
                    }
                    let mut result = EffectResult::default();
                    let label = registry
                        .get(&top.card_id)
                        .and_then(|definition| definition.face(top.face_index))
                        .map(|face| face.name.to_string())
                        .unwrap_or_else(|| top.card_id.clone());
                    let outcome = if target_context.has_bindings
                        && target_context.targets.is_empty()
                    {
                        // CR 608.2b has already taken the one stack-wide target snapshot. A
                        // authored leaf whose own owner-local role groups are now empty is
                        // skipped while its program unit and any later selected modes continue.
                        if outputs.iter().any(|slot_id| {
                            compiled.slot(*slot_id).is_some_and(|slot| {
                                matches!(slot.result_type, ResultType::DiscardResult { .. })
                            })
                        }) {
                            result.program_discard_result = Some(ProgramDiscardResult {
                                outcome: tricerules_card_model::DiscardResultOutcome::Unpayable,
                                batches: Vec::new(),
                                cards: Vec::new(),
                            });
                        }
                        if outputs.iter().any(|slot_id| {
                            compiled.slot(*slot_id).is_some_and(|slot| {
                                matches!(slot.result_type, ResultType::CounterPaymentOutcome)
                            })
                        }) {
                            result.counter_payment_outcome = Some(
                                    tricerules_card_model::primitives::CounterPaymentOutcome::NotOffered,
                                );
                        }
                        EffectOutcome::Continue
                    } else {
                        let affected_player = if compiled
                            .required_inputs
                            .contains(&ProgramRequiredInput::TriggerAffectedPlayer)
                        {
                            top.trigger_context.affected_player.ok_or(
                                EngineError::ProgramInvariant(
                                    "resolution program is missing its captured affected player",
                                ),
                            )?
                        } else {
                            top.trigger_context
                                .affected_player
                                .unwrap_or(top.controller)
                        };
                        let mut cx = EffectCx {
                            engine: self,
                            events,
                            targets: &target_context.targets,
                            targets_by_role: &target_context.targets_by_role,
                            target_damage: &target_context.target_damage,
                            target_group_indices: &target_context.target_group_indices,
                            top,
                            controller: top.controller,
                            affected_player,
                            spell_label: &label,
                            previous_effect_result: &previous,
                            resume_previous_result: Some(&previous_result),
                            effect_result: &mut result,
                            effect_index: instruction_index as u32,
                            program_state: Some(program_state.clone()),
                        };
                        if matches!(
                            &effect,
                            SpellEffectKind::SelectHandCards { .. }
                                | SpellEffectKind::PutSelectedHandCardsOnLibraryTop
                        ) {
                            run_program_only_leaf(&mut cx, effect)?
                        } else {
                            dispatch::execute_effect(&mut cx, effect)?
                        }
                    };

                    self.refresh_hideaway_look_permissions();
                    if outcome == EffectOutcome::GameEnded {
                        return Ok(ResolutionProgress::GameEnded);
                    }
                    if outcome == EffectOutcome::Suspended {
                        // The domain continuation already holds the advanced cursor and the
                        // output binding that this exact operation will fill after its commit.
                        #[cfg(test)]
                        trip_program_failpoint(ProgramFailpoint::OfferPublished)?;
                        return Ok(ResolutionProgress::Parked);
                    }
                    let resultless_blight =
                        matches!(&outcome, EffectOutcome::Blighted(_)) && outputs.is_empty();
                    if outcome != EffectOutcome::Continue && !resultless_blight {
                        return Err(EngineError::ProgramInvariant(
                            "unsupported resolution-program leaf outcome",
                        ));
                    }
                    #[cfg(test)]
                    trip_program_failpoint(ProgramFailpoint::DomainCommitted)?;
                    capture_program_outputs(compiled, &mut program_state, &outputs, &result)?;
                    program_state.in_flight = None;
                    #[cfg(test)]
                    if !outputs.is_empty() {
                        trip_program_failpoint(ProgramFailpoint::SlotPublished)?;
                    }
                    if self.finish_program_instruction_boundary(
                        top,
                        &program_state,
                        &previous_result,
                        events,
                    )? {
                        return Ok(ResolutionProgress::Parked);
                    }
                }
            }
        }
    }

    fn program_choose_requirement_holds(
        &self,
        top: &StackItem,
        requirement: &CompiledChooseRequirement,
    ) -> bool {
        match requirement {
            CompiledChooseRequirement::Always => true,
            CompiledChooseRequirement::GameCondition(condition) => {
                self.condition_holds(condition, ConditionContext::for_stack_item(top))
            }
            CompiledChooseRequirement::CastCostReceipt(condition) => {
                top.cast_cost_condition_matches(condition)
            }
        }
    }

    fn program_choose_draw_is_selectable(
        &self,
        top: &StackItem,
        previous_result: &EffectResult,
        branch: &CompiledChooseBranch,
    ) -> bool {
        if branch.cost != ResolutionCost::None || branch.body.steps.len() != 1 {
            return true;
        }
        let CompiledInstructionKind::Perform(effect) = &branch.body.steps[0].kind else {
            return true;
        };
        super::direct_draw_effect_is_selectable(
            self,
            top,
            previous_result,
            std::slice::from_ref(effect.as_ref()),
        )
        .unwrap_or(true)
    }

    /// Enter one selected branch (or the explicit otherwise block) of a parked Choose node.
    /// The occurrence is verified against the exact immutable compiled owner before a child
    /// frame is pushed, and the parent cursor must be exactly one past that instruction.
    pub(in crate::engine) fn enter_program_choose_body(
        &mut self,
        stack: &mut ParkedStackResolution,
        occurrence: &crate::state::ProgramChooseOccurrence,
        branch_id: Option<&tricerules_card_model::ChoiceId>,
    ) -> Result<(), EngineError> {
        let run = stack.program.as_mut().ok_or(EngineError::ProgramInvariant(
            "program Choose lost its parked execution state",
        ))?;
        let compiled = self.registry.compiled_program_for_owner(&run.owner).ok_or(
            EngineError::ProgramInvariant("parked Choose program owner disappeared"),
        )?;
        let parent = block_at_path(compiled, &occurrence.parent_path)?;
        let instruction = parent
            .steps
            .get(occurrence.instruction_index as usize)
            .ok_or(EngineError::ProgramInvariant(
                "parked Choose instruction disappeared",
            ))?;
        let CompiledInstructionKind::Choose {
            choice, branches, ..
        } = &instruction.kind
        else {
            return Err(EngineError::ProgramInvariant(
                "parked Choose occurrence changed instruction kind",
            ));
        };
        if choice != &occurrence.choice_id {
            return Err(EngineError::ProgramInvariant(
                "parked Choose occurrence changed choice identity",
            ));
        }
        let frame = run.frames.last().ok_or(EngineError::ProgramInvariant(
            "parked Choose lost its parent frame",
        ))?;
        if frame.path != occurrence.parent_path
            || frame.next_instruction != occurrence.instruction_index + 1
            || run.in_flight.is_some()
        {
            return Err(EngineError::ProgramInvariant(
                "parked Choose parent cursor is inconsistent",
            ));
        }
        let arm = if let Some(branch_id) = branch_id {
            if !branches.iter().any(|branch| &branch.branch_id == branch_id) {
                return Err(EngineError::ProgramInvariant(
                    "selected Choose branch id disappeared",
                ));
            }
            ProgramArm::ChooseBranch(branch_id.clone())
        } else {
            ProgramArm::ChooseOtherwise
        };
        let mut child_path = occurrence.parent_path.clone();
        child_path.push(ProgramPathStep {
            instruction_index: occurrence.instruction_index,
            arm,
        });
        run.frames.push(ProgramFrame {
            path: child_path,
            next_instruction: 0,
            yield_to: instruction.outputs.clone(),
        });
        Ok(())
    }

    pub(in crate::engine) fn program_choose_selected_branch(
        &self,
        stack: &ParkedStackResolution,
        occurrence: &crate::state::ProgramChooseOccurrence,
        original_chooser: PlayerId,
        branch_index: usize,
    ) -> Result<CompiledChooseBranch, EngineError> {
        let run = stack.program.as_ref().ok_or(EngineError::ProgramInvariant(
            "program Choose lost its parked execution state",
        ))?;
        let compiled = self.registry.compiled_program_for_owner(&run.owner).ok_or(
            EngineError::ProgramInvariant("parked Choose program owner disappeared"),
        )?;
        let parent = block_at_path(compiled, &occurrence.parent_path)?;
        let instruction = parent
            .steps
            .get(occurrence.instruction_index as usize)
            .ok_or(EngineError::ProgramInvariant(
                "parked Choose instruction disappeared",
            ))?;
        let CompiledInstructionKind::Choose {
            choice, branches, ..
        } = &instruction.kind
        else {
            return Err(EngineError::ProgramInvariant(
                "parked Choose occurrence changed instruction kind",
            ));
        };
        if choice != &occurrence.choice_id {
            return Err(EngineError::ProgramInvariant(
                "parked Choose occurrence changed choice identity",
            ));
        }
        let branch = branches
            .get(branch_index)
            .cloned()
            .ok_or(EngineError::Illegal("bad program Choose branch index"))?;
        let original_chooser_live = self
            .state
            .player_idx(original_chooser)
            .is_some_and(|idx| !self.state.players[idx].has_lost);
        if (!original_chooser_live && branch.cost != ResolutionCost::None)
            || !self.program_choose_requirement_holds(&stack.item, &branch.requirement)
            || !self.program_choose_draw_is_selectable(&stack.item, &stack.previous_result, &branch)
        {
            return Err(EngineError::Illegal(
                "that program Choose branch is no longer legal",
            ));
        }
        Ok(branch)
    }

    pub(in crate::engine) fn program_choose_choice_event(&self) -> Option<rv1::RuledEvent> {
        let pending = self.state.pending_resolution.as_ref()?;
        let ResolutionContinuation::ProgramChoose { stack, choose } = &pending.continuation else {
            return None;
        };
        let run = stack.program.as_ref()?;
        let compiled = self.registry.compiled_program_for_owner(&run.owner)?;
        let parent = block_at_path(compiled, &choose.occurrence.parent_path).ok()?;
        let instruction = parent
            .steps
            .get(choose.occurrence.instruction_index as usize)?;
        let CompiledInstructionKind::Choose {
            choice, branches, ..
        } = &instruction.kind
        else {
            return None;
        };
        if choice != &choose.occurrence.choice_id {
            return None;
        }
        let options = match &choose.stage {
            crate::state::PendingProgramChooseStage::ChoosingDelegate { candidates } => {
                let eligible = super::resolution_choice_delegate_candidates(
                    &self.state,
                    stack.item.controller,
                    choose.original_chooser,
                );
                candidates
                    .iter()
                    .enumerate()
                    .map(|(index, candidate)| {
                        let selectable = candidate.is_some_and(|candidate| {
                            eligible.contains(&candidate)
                                && self
                                    .state
                                    .player_idx(candidate)
                                    .is_some_and(|idx| !self.state.players[idx].has_lost)
                        });
                        let label = candidate.map_or_else(
                            || "Unavailable player".to_string(),
                            |candidate| format!("P{candidate} makes the choice"),
                        );
                        rv1::ResolutionBranchOption {
                            branch_index: index as u32,
                            label: label.clone(),
                            selectable,
                            presentation: Some(rv1::PresentationRef {
                                fallback_text: label,
                                ..Default::default()
                            }),
                            ..Default::default()
                        }
                    })
                    .collect()
            }
            crate::state::PendingProgramChooseStage::Selecting => {
                let original_chooser_live = self
                    .state
                    .player_idx(choose.original_chooser)
                    .is_some_and(|idx| !self.state.players[idx].has_lost);
                branches
                    .iter()
                    .enumerate()
                    .filter(|(_, branch)| {
                        (original_chooser_live || branch.cost == ResolutionCost::None)
                            && self
                                .program_choose_requirement_holds(&stack.item, &branch.requirement)
                            && self.program_choose_draw_is_selectable(
                                &stack.item,
                                &stack.previous_result,
                                branch,
                            )
                    })
                    .map(|(index, branch)| {
                        let (cost_kind, cost_text) = match &branch.cost {
                            ResolutionCost::None => {
                                (rv1::ResolutionBranchCostKind::Unspecified, String::new())
                            }
                            ResolutionCost::Mana(cost) => {
                                (rv1::ResolutionBranchCostKind::Mana, cost.to_string())
                            }
                            ResolutionCost::Waterbend(cost) => (
                                rv1::ResolutionBranchCostKind::Waterbend,
                                format!("Waterbend {cost}"),
                            ),
                            _ => return None,
                        };
                        let label = branch.fallback_label();
                        Some(rv1::ResolutionBranchOption {
                            branch_index: index as u32,
                            label: label.clone(),
                            cost_kind: cost_kind as i32,
                            cost_text,
                            selectable: true,
                            presentation: stack_child_presentation_ref(
                                self.registry,
                                &stack.item.card_id,
                                stack.item.face_index,
                                StackPresentationSource::for_stack(
                                    self.state
                                        .stack_presentations
                                        .get(&stack.item.id)
                                        .and_then(|presentation| presentation.primary.as_ref()),
                                    stack.item.ability_text.is_none(),
                                ),
                                PresentationPath::ResolutionBranch(&branch.branch_id),
                                &branch.presentation,
                                label,
                            ),
                            ..Default::default()
                        })
                    })
                    .collect::<Option<Vec<_>>>()?
            }
            crate::state::PendingProgramChooseStage::PayingMana { .. } => return None,
        };
        Some(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    deciding_player_id: pending.deciding_player,
                    source_object_id: pending.presentation.source_object_id,
                    prompt_text: pending.presentation.prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ResolutionBranch as i32,
                    min: pending.presentation.min,
                    max: pending.presentation.max,
                    resolution_branches: options,
                    ..Default::default()
                },
            )),
        })
    }

    fn finish_program_instruction_boundary(
        &mut self,
        top: &StackItem,
        program_state: &ProgramRunState,
        result: &EffectResult,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        self.refresh_hideaway_look_permissions();
        self.refresh_enduring_story_designations();
        let mut observer_stack = ParkedStackResolution::new(top.clone());
        observer_stack.program = Some(program_state.clone());
        observer_stack.previous_result = result.clone();
        if self.drain_immediate_observer_actions(Some(observer_stack), events)? {
            return Ok(true);
        }
        self.refresh_hideaway_look_permissions();
        self.check_millennium_calendar_state_triggers();
        Ok(false)
    }
}

#[derive(Default)]
struct ProgramTargetContext {
    has_bindings: bool,
    targets: Vec<ObjectId>,
    targets_by_role: Vec<Vec<ObjectId>>,
    target_damage: Vec<u32>,
    target_group_indices: Vec<u32>,
}

/// Project the exact target records owned by one compiled instruction address. Mode-local
/// programs read only their stable mode's target records and never fall back to the stack-wide
/// flattened target list.
fn program_target_context(
    top: &StackItem,
    compiled: &CompiledProgram,
    instruction_address: u32,
) -> Result<ProgramTargetContext, EngineError> {
    if compiled.target_bindings.is_empty() {
        return Ok(ProgramTargetContext::default());
    }
    let mut bindings = compiled
        .target_bindings
        .iter()
        .filter(|binding| binding.instruction_address == instruction_address)
        .collect::<Vec<_>>();
    if bindings.is_empty() {
        return Ok(ProgramTargetContext::default());
    }
    bindings.sort_by_key(|binding| binding.role_index);
    if bindings
        .iter()
        .enumerate()
        .any(|(role_index, binding)| binding.role_index as usize != role_index)
    {
        return Err(EngineError::ProgramInvariant(
            "source target bindings have a missing or repeated role index",
        ));
    }

    let chosen_targets = match &compiled.target_projection {
        tricerules_card_model::primitives::ProgramTargetProjection::OwnerLocal => {
            top.targets.as_slice()
        }
        tricerules_card_model::primitives::ProgramTargetProjection::ModeLocal { mode_id } => top
            .chosen_modes
            .iter()
            .find(|chosen| chosen.mode_id == *mode_id)
            .map(|chosen| chosen.targets.as_slice())
            .ok_or(EngineError::ProgramInvariant(
                "mode-local target program has no matching chosen mode",
            ))?,
    };
    let role_group_indices = bindings
        .iter()
        .map(|binding| binding.target_group_index)
        .collect::<Vec<_>>();
    let selected_targets = chosen_targets
        .iter()
        .filter(|target| role_group_indices.contains(&target.group_index))
        .collect::<Vec<_>>();
    let targets_by_role = bindings
        .iter()
        .map(|binding| {
            chosen_targets
                .iter()
                .filter(|target| target.group_index == binding.target_group_index)
                .map(|target| target.object_id)
                .collect()
        })
        .collect();
    Ok(ProgramTargetContext {
        has_bindings: true,
        targets: selected_targets
            .iter()
            .map(|target| target.object_id)
            .collect(),
        targets_by_role,
        target_damage: selected_targets
            .iter()
            .map(|target| target.damage_amount)
            .collect(),
        target_group_indices: selected_targets
            .iter()
            .map(|target| target.group_index)
            .collect(),
    })
}

fn block_at_path<'a>(
    program: &'a CompiledProgram,
    path: &[ProgramPathStep],
) -> Result<&'a CompiledBlock, EngineError> {
    let mut block = &program.body;
    for step in path {
        let instruction = block.steps.get(step.instruction_index as usize).ok_or(
            EngineError::ProgramInvariant("resolution program frame path is invalid"),
        )?;
        block = match (&instruction.kind, &step.arm) {
            (CompiledInstructionKind::If { yes, .. }, ProgramArm::Yes) => yes,
            (CompiledInstructionKind::If { no, .. }, ProgramArm::No) => no,
            (
                CompiledInstructionKind::Choose { branches, .. },
                ProgramArm::ChooseBranch(branch_id),
            ) => branches
                .iter()
                .find(|branch| &branch.branch_id == branch_id)
                .map(|branch| &branch.body)
                .ok_or(EngineError::ProgramInvariant(
                    "resolution program Choose branch path is invalid",
                ))?,
            (CompiledInstructionKind::Choose { otherwise, .. }, ProgramArm::ChooseOtherwise) => {
                otherwise
            }
            _ => {
                return Err(EngineError::ProgramInvariant(
                    "resolution program frame is not a branch",
                ))
            }
        };
    }
    Ok(block)
}

fn capture_program_outputs(
    compiled: &CompiledProgram,
    run: &mut ProgramRunState,
    outputs: &[ResultSlotId],
    result: &EffectResult,
) -> Result<(), EngineError> {
    for slot_id in outputs {
        let slot = compiled
            .slot(*slot_id)
            .ok_or(EngineError::ProgramInvariant(
                "resolution program output slot is invalid",
            ))?;
        let value = match &slot.result_type {
            ResultType::DrawReceipt { .. } => {
                ProgramValue::DrawReceipt(result.produced_objects.clone())
            }
            ResultType::DiscardResult { .. } => {
                ProgramValue::DiscardResult(result.program_discard_result.clone().ok_or(
                    EngineError::ProgramInvariant(
                        "discard instruction did not publish its typed result",
                    ),
                )?)
            }
            ResultType::CounterPaymentOutcome => {
                ProgramValue::CounterPaymentOutcome(result.counter_payment_outcome.ok_or(
                    EngineError::ProgramInvariant(
                        "counter instruction did not publish its typed payment outcome",
                    ),
                )?)
            }
            ResultType::PermanentSelection { .. } => {
                ProgramValue::PermanentSelection(result.produced_objects.clone())
            }
            ResultType::HandSelection { .. } => {
                ProgramValue::HandSelection(result.selected_hand_cards.clone())
            }
            ResultType::CardResultCohort { .. } => {
                ProgramValue::CardResultCohort(crate::state::CardResultCohort {
                    cards: result.cards.clone(),
                    program_discard_result: None,
                })
            }
            ResultType::FoundLibraryCohort => match result.program_value.as_ref() {
                Some(value @ ProgramValue::FoundLibraryCohort(_)) => value.clone(),
                _ => {
                    return Err(EngineError::ProgramInvariant(
                        "retained library search did not publish its exact cohort",
                    ))
                }
            },
            ResultType::RevealedLibraryCohort => match result.program_value.as_ref() {
                Some(value @ ProgramValue::RevealedLibraryCohort(_)) => value.clone(),
                _ => {
                    return Err(EngineError::ProgramInvariant(
                        "cohort reveal did not publish its typed result",
                    ))
                }
            },
            ResultType::CardPartition => match result.program_value.as_ref() {
                Some(value @ ProgramValue::CardPartition(_)) => value.clone(),
                _ => {
                    return Err(EngineError::ProgramInvariant(
                        "cohort split did not publish its typed partition",
                    ))
                }
            },
        };
        let destination =
            run.slots
                .get_mut(slot_id.0 as usize)
                .ok_or(EngineError::ProgramInvariant(
                    "resolution program output slot is outside the frame",
                ))?;
        *destination = Some(value);
    }
    Ok(())
}

fn validate_program_unit_inputs(
    top: &StackItem,
    compiled: &CompiledProgram,
) -> Result<(), EngineError> {
    if compiled
        .required_inputs
        .contains(&ProgramRequiredInput::TriggerAffectedPlayer)
        && top.trigger_context.affected_player.is_none()
    {
        return Err(EngineError::ProgramInvariant(
            "resolution program is missing its captured affected player",
        ));
    }
    Ok(())
}

fn program_result_condition_holds(
    state: &crate::state::GameState,
    top: &StackItem,
    run: &ProgramRunState,
    condition: &CompiledProgramResultCondition,
) -> Result<bool, EngineError> {
    match condition {
        CompiledProgramResultCondition::CounterPayment { result, outcome } => {
            let Some(ProgramValue::CounterPaymentOutcome(actual)) =
                run.slots.get(result.0 as usize).and_then(Option::as_ref)
            else {
                return Err(EngineError::ProgramInvariant(
                    "counter payment condition read an unavailable or wrong-typed slot",
                ));
            };
            Ok(actual == outcome)
        }
        CompiledProgramResultCondition::DiscardOutcome { result, outcome } => {
            let Some(ProgramValue::DiscardResult(result)) =
                run.slots.get(result.0 as usize).and_then(Option::as_ref)
            else {
                return Err(EngineError::ProgramInvariant(
                    "discard outcome condition read an unavailable or wrong-typed slot",
                ));
            };
            Ok(result.outcome == *outcome)
        }
        CompiledProgramResultCondition::DiscardCount {
            result,
            players,
            card_type,
            min,
            max,
        } => {
            let Some(ProgramValue::DiscardResult(result)) =
                run.slots.get(result.0 as usize).and_then(Option::as_ref)
            else {
                return Err(EngineError::ProgramInvariant(
                    "discard count condition read an unavailable or wrong-typed slot",
                ));
            };
            let count = result
                .cards
                .iter()
                .filter(|card| {
                    crate::engine::history::relative_player_set_contains(
                        state,
                        *players,
                        top.controller,
                        card.affected_player,
                    )
                })
                .filter(|card| match (&card.type_knowledge, card_type) {
                    (_, None) => true,
                    (DiscardCardTypeKnowledge::Known(types), Some(kind)) => types.contains(kind),
                    (DiscardCardTypeKnowledge::UnknownHidden, Some(_)) => false,
                })
                .count()
                .min(u32::MAX as usize) as u32;
            Ok(min.is_none_or(|minimum| count >= minimum)
                && max.is_none_or(|maximum| count <= maximum))
        }
    }
}

fn run_program_only_leaf(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    match effect {
        SpellEffectKind::SelectHandCards {
            count,
            ordered,
            prompt,
        } => {
            let amount = cx.engine.resolve_amount(
                &count,
                AmountContext::for_stack_item(cx.top, cx.top.controller),
            );
            super::zones::select_hand_cards_for_program(cx, amount, ordered, &prompt)
        }
        SpellEffectKind::PutSelectedHandCardsOnLibraryTop => {
            let engine = &mut *cx.engine;
            let top = cx.top;
            let selected = &cx.previous_effect_result.selected_hand_cards;
            let mut ordered = Vec::with_capacity(selected.len());
            for selected in selected {
                let Some(object) = engine.state.objects.get(&selected.object_id) else {
                    return Err(EngineError::Illegal(
                        "selected hand card disappeared before library placement",
                    ));
                };
                let Some(index) = engine.state.player_idx(object.owner) else {
                    return Err(EngineError::Illegal("selected hand card owner disappeared"));
                };
                if object.zone != Zone::Hand
                    || !engine.state.players[index]
                        .hand
                        .contains(&selected.object_id)
                    || engine
                        .state
                        .zone_change_generation
                        .get(&selected.object_id)
                        .copied()
                        .unwrap_or(0)
                        != selected.zone_change_generation
                    || object.owner != top.controller
                {
                    return Err(EngineError::Illegal(
                        "selected hand card became stale before library placement",
                    ));
                }
                ordered.push(selected.object_id);
            }
            // The choice sequence is bottom-to-top; each later selected card is placed above
            // those selected earlier, matching the existing Brainstorm contract.
            for object_id in &ordered {
                move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    *object_id,
                    Zone::Library,
                    None,
                )?;
                let index = engine
                    .state
                    .player_idx(top.controller)
                    .ok_or(EngineError::Illegal(
                        "resolution-program controller disappeared",
                    ))?;
                let library = &mut engine.state.players[index].library;
                library.retain(|other| *other != *object_id);
                library.push_front(*object_id);
            }
            if !ordered.is_empty() {
                cx.events.push(ev_log(format!(
                    "P{} puts {} card{} on top of library ({}).",
                    top.controller,
                    ordered.len(),
                    if ordered.len() == 1 { "" } else { "s" },
                    cx.spell_label,
                )));
            }
            Ok(EffectOutcome::Continue)
        }
        _ => Err(EngineError::Illegal(
            "operation is not a resolution-program-only leaf",
        )),
    }
}

#[cfg(test)]
mod invariant_rollback_tests {
    use super::*;

    #[derive(PartialEq, Eq)]
    struct CommandSnapshot {
        diagnostic: serde_json::Value,
        command_index: u64,
        private_zones: HashMap<PlayerId, PrivateZoneSnapshot>,
        battlefield: Option<BattlefieldViewSnapshot>,
        first_strike: bool,
        pending_spell: bool,
        pending_ability: bool,
        pending_counter_cost: bool,
        pending_attack_declaration: bool,
    }

    fn command_snapshot(engine: &GameEngine) -> CommandSnapshot {
        CommandSnapshot {
            diagnostic: engine.diagnostic_snapshot().expect("diagnostic snapshot"),
            command_index: engine.state.command_index,
            private_zones: engine.private_zone_cache.clone(),
            battlefield: engine.battlefield_view_cache.clone(),
            first_strike: engine.first_strike_step_pending_cache,
            pending_spell: engine.pending_spell_cast_internal.is_some(),
            pending_ability: engine.pending_ability_activation_internal.is_some(),
            pending_counter_cost: engine.pending_counter_cost_internal.is_some(),
            pending_attack_declaration: engine.pending_attack_declaration_internal.is_some(),
        }
    }

    fn program_resolution_fixture() -> GameEngine {
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            91_204,
            &[0, 1],
            20,
        )
        .expect("game engine");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;

        let spell = *engine.state.players[0]
            .hand
            .first()
            .expect("default deck dealt a hand card");
        engine
            .state
            .objects
            .get_mut(&spell)
            .expect("hand object")
            .card_id = "brainstorm".into();
        move_object_to_zone(&mut engine.state, engine.registry, spell, Zone::Stack, None)
            .expect("put spell on stack");
        let generation = engine
            .state
            .zone_change_generation
            .get(&spell)
            .copied()
            .unwrap_or(0);
        let mut item = engine.observer_return_item(spell, 0);
        item.id = spell;
        item.controller = 0;
        item.card_id = "brainstorm".into();
        item.ability_text = None;
        item.is_copy = false;
        item.face_index = 0;
        item.source_owner = Some(0);
        item.cast_by = Some(0);
        item.cast_occurrence = Some(crate::state::StackObjectRef {
            object_id: spell,
            zone_change_generation: Some(generation),
        });
        engine.state.stack.push(item);

        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("first player passes");
        assert_eq!(engine.state.priority_player_id(), 1);
        engine
    }

    fn static_grant_modal_program_fixture() -> (
        GameEngine,
        tricerules_card_model::TriggeredAbilityDef,
        tricerules_card_model::AbilityDefinitionId,
        Vec<tricerules_card_model::ModeId>,
    ) {
        let card = r#"(
            id: "static_modal_grant",
            name: "Static Modal Grant",
            face_id: "static_modal_grant_face",
            types: ["Creature"], power: 2, toughness: 2,
            static_abilities: [(
                ability_id: "static_01", presentation: Fallback,
                definition: GrantTriggeredAbilityToPermanents(
                    filter: (kind: Creature, controller: You), condition: None,
                    triggered_abilities: [(
                        ability_id: "triggered_01", presentation: Fallback,
                        trigger: WhenSelfEntersBattlefield,
                        modal: Some((min_modes: 2, max_modes: 2, modes: [
                            (mode_id: "first_mode", presentation: Fallback,
                                effects: [],
                                program: Some((body: (steps: [
                                    (id: "damage", kind: Perform(DamageTarget(amount: 1,
                                        target: (kind: Creature)))),
                                    (id: "discard", bind: ["discarded"], kind: Perform(Discard(
                                        who: Controller, quantity: OptionalExact(1)
                                    ))),
                                    (id: "draw_if_paid", kind: IfResult(
                                        condition: DiscardOutcome(result: "discarded",
                                            outcome: ChosenToPay),
                                        yes: (steps: [(id: "draw", kind: Perform(Draw(
                                            who: Controller, count: 1
                                        )))], yields: []),
                                        no: (steps: [], yields: [])
                                    ))
                                ], yields: []), target_bindings: [
                                    (instruction_id: "damage", role: Target,
                                        target_group_index: 0)
                                ])),
                                targeting: Some((groups: [(min: 1, max: 1,
                                    prompt: "Choose first creature", effect_indices: [])]))),
                            (mode_id: "second_mode", presentation: Fallback,
                                effects: [],
                                program: Some((body: (steps: [
                                    (id: "damage", kind: Perform(DamageTarget(amount: 1,
                                        target: (kind: Creature)))),
                                    (id: "discard", bind: ["discarded"], kind: Perform(Discard(
                                        who: Controller, quantity: OptionalExact(1)
                                    ))),
                                    (id: "draw_if_paid", kind: IfResult(
                                        condition: DiscardOutcome(result: "discarded",
                                            outcome: ChosenToPay),
                                        yes: (steps: [(id: "draw", kind: Perform(Draw(
                                            who: Controller, count: 1
                                        )))], yields: []),
                                        no: (steps: [], yields: [])
                                    ))
                                ], yields: []), target_bindings: [
                                    (instruction_id: "damage", role: Target,
                                        target_group_index: 0)
                                ])),
                                targeting: Some((groups: [(min: 1, max: 1,
                                    prompt: "Choose second creature", effect_indices: [])])))
                        ])),
                    )],
                ),
            )],
        )"#;
        let registry = Box::leak(Box::new(
            tricerules_card_model::CardRegistry::from_chunks_and_tokens(
                &[
                    card,
                    include_str!("../../../../tricerules-cards/data/island.ron"),
                    include_str!("../../../../tricerules-cards/data/forest.ron"),
                ],
                &[],
            )
            .expect("static modal grant fixture registry validates"),
        ));
        let decks = Some(vec![
            vec!["static_modal_grant".into(); 30],
            vec!["island".into(); 30],
        ]);
        let engine = GameEngine::new(registry, 91_205, &[0, 1], 20, decks, true)
            .expect("static modal grant fixture engine");
        let face = registry
            .get("static_modal_grant")
            .and_then(|card| card.face(0))
            .expect("fixture face");
        let (grant_ability_id, ability) = match &face.static_abilities[0].definition {
            tricerules_card_model::primitives::StaticAbilityDef::GrantTriggeredAbilityToPermanents {
                triggered_abilities,
                ..
            } => (
                face.static_abilities[0].ability_id.clone(),
                triggered_abilities[0].clone(),
            ),
            other => panic!("unexpected static grant fixture: {other:?}"),
        };
        let owner = tricerules_card_model::AbilityDefinitionId {
            card_id: "static_modal_grant".into(),
            face_id: face.face_id.clone(),
            ability_path: vec![grant_ability_id, ability.ability_id.clone()],
        };
        let modes = ability
            .modal
            .as_ref()
            .expect("modal grant definition")
            .modes
            .iter()
            .map(|mode| mode.mode_id.clone())
            .collect();
        (engine, ability, owner, modes)
    }

    fn static_grant_modal_stack_item(
        engine: &GameEngine,
        source_id: ObjectId,
        ability: &tricerules_card_model::TriggeredAbilityDef,
        definition: &tricerules_card_model::AbilityDefinitionId,
        modes: &[tricerules_card_model::ModeId],
        capture_body: bool,
    ) -> StackItem {
        use crate::state::{ChosenMode, StackTarget, TriggerAbilityOrigin, TriggerUseKey};

        let generation = engine
            .state
            .zone_change_generation
            .get(&source_id)
            .copied()
            .unwrap_or(0);
        let mut top = engine.observer_return_item(source_id, 0);
        top.id = engine.state.next_object_id.saturating_add(1);
        top.card_id = "static_modal_grant".into();
        top.ability_text = Some("static modal grant".into());
        top.source_permanent_id = Some(source_id);
        top.source_owner = Some(0);
        top.source_zone_change = generation;
        top.trigger_use_key = Some(TriggerUseKey {
            object_id: source_id,
            zone_change_generation: generation,
            ability_origin: TriggerAbilityOrigin::StaticGrant {
                source_id,
                source_zone_change: generation,
                definition: definition.clone(),
            },
        });
        top.ability_index = Some(usize::MAX);
        top.triggered_ability = capture_body.then(|| ability.clone());
        top.is_triggered = true;
        top.is_copy = false;
        top.chosen_modes = modes
            .iter()
            .map(|mode_id| ChosenMode {
                mode_id: mode_id.clone(),
                // A hand object is deliberately used as a stale target. If target snapshot/fizzle
                // runs before owner validation, both selected modes would be consumed here.
                targets: vec![StackTarget {
                    object_id: source_id,
                    group_index: 0,
                    damage_amount: 0,
                    kind: rv1::TargetRefKind::Permanent as i32,
                    zone_change_generation: Some(generation),
                    required_controller: None,
                }],
            })
            .collect();
        top
    }

    fn pass_priority() -> RuledCommand {
        RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
        }
    }

    fn choose_first_two_offered_hand_cards(engine: &GameEngine) -> RuledCommand {
        let chosen_object_ids = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Brainstorm hand choice is pending")
            .presentation
            .candidates
            .iter()
            .take(2)
            .copied()
            .collect();
        RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    chosen_object_ids,
                    ..Default::default()
                },
            )),
        }
    }

    #[test]
    fn program_invariant_failure_restores_full_command_and_retry_matches_control() {
        for point in [
            ProgramFailpoint::DomainCommitted,
            ProgramFailpoint::OfferPublished,
        ] {
            let mut engine = program_resolution_fixture();
            let mut control = engine.clone();
            let before = command_snapshot(&engine);

            let failure = {
                let _guard = ProgramFailpointGuard::arm(point);
                engine.apply_scenario_command(1, &pass_priority())
            };
            assert!(
                matches!(failure, Err(EngineError::ProgramInvariant(_))),
                "expected the {point:?} one-shot failure, got {failure:?}"
            );
            assert!(
                command_snapshot(&engine) == before,
                "the {point:?} failure must restore public state, caches, pending internals, and command index"
            );

            let retried = engine
                .apply_scenario_command(1, &pass_priority())
                .expect("failed command can be retried");
            let accepted = control
                .apply_scenario_command(1, &pass_priority())
                .expect("control command succeeds");
            assert_eq!(
                retried, accepted,
                "retry after {point:?} must match the no-fault batch"
            );
            assert!(command_snapshot(&engine) == command_snapshot(&control));
        }
    }

    #[test]
    fn program_choice_resume_failure_restores_prompt_and_retry_matches_control() {
        for point in [
            ProgramFailpoint::DomainCommitted,
            ProgramFailpoint::SlotPublished,
        ] {
            let mut engine = program_resolution_fixture();
            engine
                .apply_scenario_command(1, &pass_priority())
                .expect("Brainstorm draws three and parks on hand choice");
            let mut control = engine.clone();
            let answer = choose_first_two_offered_hand_cards(&engine);
            let before = command_snapshot(&engine);

            let failure = {
                let _guard = ProgramFailpointGuard::arm(point);
                engine.apply_scenario_command(0, &answer)
            };
            assert!(
                matches!(failure, Err(EngineError::ProgramInvariant(_))),
                "expected the {point:?} one-shot failure during choice resumption"
            );
            assert!(
                command_snapshot(&engine) == before,
                "the {point:?} resumed failure must restore the exact prompt, program frame, and already-drawn state"
            );

            let retried = engine
                .apply_scenario_command(0, &answer)
                .expect("the same hand-choice answer can be retried");
            let accepted = control
                .apply_scenario_command(0, &answer)
                .expect("control hand-choice answer succeeds");
            assert_eq!(
                retried, accepted,
                "retry after {point:?} must match the no-fault batch"
            );
            assert!(command_snapshot(&engine) == command_snapshot(&control));
        }
    }

    #[test]
    fn chosen_to_pay_outcome_is_independent_of_an_empty_discard_receipt_cohort() {
        let engine = program_resolution_fixture();
        let top = engine.state.stack.last().expect("resolving spell").clone();
        let result_slot = ResultSlotId(0);
        let run = ProgramRunState {
            owner: tricerules_card_model::ProgramOwnerId::face(
                "brainstorm",
                tricerules_card_model::CardFaceId::new("brainstorm").unwrap(),
            ),
            pending_owners: Vec::new(),
            frames: vec![ProgramFrame {
                path: Vec::new(),
                next_instruction: 0,
                yield_to: Vec::new(),
            }],
            slots: vec![Some(ProgramValue::DiscardResult(ProgramDiscardResult {
                outcome: tricerules_card_model::DiscardResultOutcome::ChosenToPay,
                batches: Vec::new(),
                cards: Vec::new(),
            }))],
            in_flight: None,
        };

        assert!(program_result_condition_holds(
            &engine.state,
            &top,
            &run,
            &CompiledProgramResultCondition::DiscardOutcome {
                result: result_slot,
                outcome: tricerules_card_model::DiscardResultOutcome::ChosenToPay,
            },
        )
        .expect("outcome condition evaluates"));
        assert!(!program_result_condition_holds(
            &engine.state,
            &top,
            &run,
            &CompiledProgramResultCondition::DiscardCount {
                result: result_slot,
                players: tricerules_card_model::primitives::RelativePlayerSet::Controller,
                card_type: Some(tricerules_card_model::primitives::CardTypeFilter::Nonland),
                min: Some(1),
                max: None,
            },
        )
        .expect("count condition evaluates"));
    }

    #[test]
    fn anvil_program_requires_its_captured_affected_player_before_running() {
        use tricerules_card_model::ProgramOwnerId;

        let engine = program_resolution_fixture();
        let face = engine
            .registry
            .get("anvil_of_bogardan")
            .and_then(|card| card.face(0))
            .expect("Anvil face");
        let ability = face
            .triggered_abilities
            .first()
            .expect("Anvil draw-step trigger");
        let owner = ProgramOwnerId::ability(tricerules_card_model::AbilityDefinitionId {
            card_id: "anvil_of_bogardan".into(),
            face_id: face.face_id.clone(),
            ability_path: vec![ability.ability_id.clone()],
        });
        let compiled = engine
            .registry
            .compiled_program_for_owner(&owner)
            .expect("Anvil trigger program was compiled");
        assert!(compiled
            .required_inputs
            .contains(&ProgramRequiredInput::TriggerAffectedPlayer));

        let mut top = engine
            .state
            .stack
            .last()
            .expect("fixture stack item")
            .clone();
        assert!(matches!(
            validate_program_unit_inputs(&top, compiled),
            Err(EngineError::ProgramInvariant(
                "resolution program is missing its captured affected player"
            ))
        ));

        top.trigger_context.affected_player = Some(1);
        validate_program_unit_inputs(&top, compiled)
            .expect("the captured draw-step actor satisfies the input guard");
    }

    #[test]
    fn modal_trigger_owner_requires_its_captured_static_grant_definition_before_fizzle() {
        use tricerules_card_model::ProgramOwnerId;

        let (mut engine, ability, definition, modes) = static_grant_modal_program_fixture();
        let source_id = engine.state.players[0].hand[0];
        let missing_capture =
            static_grant_modal_stack_item(&engine, source_id, &ability, &definition, &modes, false);
        assert_eq!(
            missing_capture.ability_program_owner(),
            Some(ProgramOwnerId::ability(definition.clone()))
        );
        engine.state.stack.push(missing_capture);
        let mut events = Vec::new();
        let error = engine
            .resolve_top_of_stack(&mut events)
            .expect_err("a compiled targeted modal trigger cannot recover its missing body");
        assert!(
            matches!(
                error,
                EngineError::ProgramInvariant(
                    "compiled ability program is missing its captured definition"
                )
            ),
            "{error:?}"
        );
        assert!(
            events.is_empty(),
            "the invariant precedes the global fizzle"
        );

        let captured =
            static_grant_modal_stack_item(&engine, source_id, &ability, &definition, &modes, true);
        assert_eq!(
            engine
                .compiled_resolution_program_owners(&captured)
                .expect("captured static grant owner validates"),
            Some(
                modes
                    .iter()
                    .map(|mode_id| {
                        ProgramOwnerId::ability_mode(definition.clone(), mode_id.clone())
                    })
                    .collect::<Vec<_>>()
            ),
            "static-granted modal units use the complete captured ability path, not the face index"
        );
    }

    #[test]
    fn captured_program_abilities_require_their_exact_registry_owner() {
        let engine = program_resolution_fixture();
        let uthros = engine
            .registry
            .get("uthros_scanship")
            .and_then(|card| card.face(0))
            .expect("Uthros Scanship face");
        let trigger_definition = uthros
            .triggered_abilities
            .iter()
            .find(|ability| ability.program.is_some())
            .expect("Uthros authored trigger")
            .clone();
        let mut trigger = engine
            .state
            .stack
            .last()
            .expect("fixture stack item")
            .clone();
        trigger.card_id = "uthros_scanship".into();
        trigger.ability_text = Some("Uthros ETB trigger".into());
        trigger.is_triggered = true;
        trigger.triggered_ability = Some(trigger_definition.clone());
        trigger.trigger_use_key = None;
        assert_eq!(trigger.ability_program_owner(), None);
        assert!(matches!(
            engine.compiled_resolution_program_owners(&trigger),
            Err(EngineError::ProgramInvariant(
                "captured authored ability program has no exact owner occurrence"
            ))
        ));

        let agna = engine
            .registry
            .get("agna_qela")
            .and_then(|card| card.face(0))
            .expect("Agna face");
        let activation_definition = agna
            .activated_abilities
            .iter()
            .find(|ability| ability.program.is_some())
            .expect("Agna authored activation")
            .clone();
        let mut activation = engine
            .state
            .stack
            .last()
            .expect("fixture stack item")
            .clone();
        activation.card_id = "agna_qela".into();
        activation.ability_text = Some("Agna loot activation".into());
        activation.is_triggered = false;
        activation.activated_ability = Some(activation_definition.clone());
        activation.activated_ability_occurrence = None;
        assert!(matches!(
            engine.compiled_resolution_program_owners(&activation),
            Err(EngineError::ProgramInvariant(
                "captured authored ability program has no exact owner occurrence"
            ))
        ));

        let anvil = engine
            .registry
            .get("anvil_of_bogardan")
            .and_then(|card| card.face(0))
            .expect("Anvil face");
        let mut legacy = trigger;
        legacy.card_id = "anvil_of_bogardan".into();
        let mut legacy_ability = anvil
            .triggered_abilities
            .first()
            .expect("Anvil trigger")
            .clone();
        // Legacy effect-only triggers still use ordinary effect-list dispatch without acquiring
        // a compiled program owner.
        legacy_ability.program = None;
        legacy_ability.effect = vec![tricerules_card_model::primitives::SpellEffectKind::Draw {
            who: tricerules_card_model::primitives::PlayerRecipient::Controller,
            count: tricerules_card_model::primitives::Amount::Fixed(1),
        }];
        legacy.triggered_ability = Some(legacy_ability);
        assert!(legacy.triggered_ability.as_ref().unwrap().program.is_none());
        assert_eq!(
            engine
                .compiled_resolution_program_owners(&legacy)
                .expect("unowned legacy trigger retains fallback"),
            None
        );
    }

    #[test]
    fn unchanged_program_choose_delegate_refresh_preserves_prompt_and_menu() {
        use crate::state::PendingProgramChooseStage;

        let card = r#"(id: "program_choose_delegate_fixture", name: "Program Choose Delegate Fixture",
            face_id: "program_choose_delegate_fixture", mana_cost: "{0}", types: ["Instant"],
            spell_program: Some((body: (steps: [
                (id: "choose", bind: [], input: None, kind: Choose(
                    choice: "choose", chooser: Controller, optional: false,
                    branches: [
                        (branch_id: "first", presentation: Fallback, cost: None,
                            requirement: Always, body: (steps: [], yields: [])),
                        (branch_id: "second", presentation: OracleLines([2]), cost: None,
                            requirement: Always, body: (steps: [], yields: [])),
                    ],
                    otherwise: (steps: [], yields: []),
                ))
            ], yields: []))))"#;
        let registry = Box::leak(Box::new(
            tricerules_card_model::CardRegistry::from_chunks_and_tokens(&[card], &[])
                .expect("all-free Choose fixture compiles"),
        ));
        let decks = Some(
            (0..4)
                .map(|_| vec!["program_choose_delegate_fixture".to_string(); 30])
                .collect(),
        );
        let mut engine = GameEngine::new(registry, 91_205, &[0, 1, 2, 3], 20, decks, true)
            .expect("four-player program Choose fixture");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;
        let spell = *engine.state.players[0]
            .hand
            .first()
            .expect("fixture deck dealt a card");
        move_object_to_zone(&mut engine.state, engine.registry, spell, Zone::Stack, None)
            .expect("fixture spell moves to stack");
        let generation = engine
            .state
            .zone_change_generation
            .get(&spell)
            .copied()
            .unwrap_or(0);
        let mut top = engine.observer_return_item(spell, 0);
        top.id = spell;
        top.controller = 0;
        top.card_id = "program_choose_delegate_fixture".into();
        top.ability_text = None;
        top.is_copy = false;
        top.cast_by = Some(0);
        top.cast_occurrence = Some(crate::state::StackObjectRef {
            object_id: spell,
            zone_change_generation: Some(generation),
        });
        let owners = engine
            .compiled_resolution_program_owners(&top)
            .expect("fixture owner lookup")
            .expect("fixture program owner");
        let run = engine
            .begin_program_run(&top, owners)
            .expect("fixture program starts");
        let mut initial = Vec::new();
        assert!(matches!(
            engine
                .run_program_with_previous(
                    &top,
                    run,
                    crate::state::EffectResult::default(),
                    &mut initial,
                )
                .expect("program reaches Choose"),
            ResolutionProgress::Parked
        ));

        // Model the source/controller leaving after its stack ability began. The next live
        // player selects a delegate, and a repeated lifecycle refresh must remain a no-op.
        engine.state.players[0].has_lost = true;
        let mut departed = Vec::new();
        engine
            .refresh_program_choose_departure(&mut departed)
            .expect("first refresh offers delegate selection");
        engine
            .publish_resolution_prompt_ids(&mut departed)
            .expect("delegate prompt gets its stable ID");
        assert!(matches!(
            &engine.state.pending_resolution.as_ref().unwrap().continuation,
            ResolutionContinuation::ProgramChoose {
                choose: crate::state::PendingProgramChoose {
                    stage: PendingProgramChooseStage::ChoosingDelegate { candidates },
                    ..
                },
                ..
            } if candidates == &[Some(1), Some(2), Some(3)]
        ));
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        let prompt_id = pending.resolution_prompt_id;
        let deciding_player = pending.deciding_player;
        let prompt_text = pending.presentation.prompt.clone();
        let published = engine
            .program_choose_choice_event()
            .expect("delegate offer renders");
        let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(original_menu)) =
            published.ev.as_ref()
        else {
            panic!("delegate offer contains its stable branch menu")
        };
        let second = original_menu
            .resolution_branches
            .iter()
            .find(|branch| branch.branch_index == 1)
            .expect("second branch remains at its authored index");
        assert_eq!(second.label, "P2 makes the choice");
        let second_presentation = second
            .presentation
            .as_ref()
            .expect("delegate candidate has a presentation reference");
        assert_eq!(second_presentation.fallback_text, "P2 makes the choice");

        let mut repeated = Vec::new();
        engine
            .refresh_program_choose_departure(&mut repeated)
            .expect("unchanged lifecycle refresh succeeds");
        assert!(
            repeated.is_empty(),
            "no-op refresh emits no replacement offer"
        );
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(pending.resolution_prompt_id, prompt_id);
        assert_eq!(pending.deciding_player, deciding_player);
        assert_eq!(pending.presentation.prompt, prompt_text);
        assert!(matches!(
            &pending.continuation,
            ResolutionContinuation::ProgramChoose {
                choose: crate::state::PendingProgramChoose {
                    stage: PendingProgramChooseStage::ChoosingDelegate { candidates },
                    ..
                },
                ..
            } if candidates == &[Some(1), Some(2), Some(3)]
        ));
        assert_eq!(engine.program_choose_choice_event(), Some(published));
    }

    fn nested_choose_fixture(
        card: &str,
        card_id: &str,
        seed: u64,
        tapped_islands: usize,
        chosen_modes: &[&str],
    ) -> (
        GameEngine,
        StackItem,
        Vec<tricerules_card_model::ProgramOwnerId>,
        Vec<ObjectId>,
    ) {
        let registry = Box::leak(Box::new(
            tricerules_card_model::CardRegistry::from_chunks_and_tokens(
                &[
                    card,
                    include_str!("../../../../tricerules-cards/data/island.ron"),
                    include_str!("../../../../tricerules-cards/data/forest.ron"),
                ],
                &[],
            )
            .expect("nested Choose fixture registry"),
        ));
        let decks = Some(vec![
            vec![card_id.to_string(); 30],
            vec!["island".to_string(); 30],
        ]);
        let mut engine = GameEngine::new(registry, seed, &[0, 1], 20, decks, true)
            .expect("nested Choose fixture engine");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;

        let island_ids = engine.state.players[1]
            .library
            .iter()
            .take(tapped_islands)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(island_ids.len(), tapped_islands);
        for island in &island_ids {
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                *island,
                Zone::Battlefield,
                Some(0),
            )
            .expect("fixture Island enters under player zero's control");
            engine.state.objects.get_mut(island).unwrap().tapped = true;
        }

        let spell = *engine.state.players[0]
            .hand
            .first()
            .expect("fixture spell in hand");
        move_object_to_zone(&mut engine.state, engine.registry, spell, Zone::Stack, None)
            .expect("fixture spell moves to stack");
        let generation = engine
            .state
            .zone_change_generation
            .get(&spell)
            .copied()
            .unwrap_or(0);
        let mut top = engine.observer_return_item(spell, 0);
        top.id = spell;
        top.controller = 0;
        top.card_id = card_id.into();
        top.ability_text = None;
        top.is_copy = false;
        top.face_index = 0;
        top.source_owner = Some(0);
        top.cast_by = Some(0);
        top.cast_occurrence = Some(crate::state::StackObjectRef {
            object_id: spell,
            zone_change_generation: Some(generation),
        });
        top.chosen_modes = chosen_modes
            .iter()
            .map(|mode| crate::state::ChosenMode {
                mode_id: tricerules_card_model::ModeId::new(*mode).expect("mode ID"),
                targets: Vec::new(),
            })
            .collect();
        let owners = engine
            .compiled_resolution_program_owners(&top)
            .expect("program owner lookup")
            .expect("program owner exists");
        engine.state.stack.push(top.clone());
        (engine, top, owners, island_ids)
    }

    fn nested_start(
        engine: &mut GameEngine,
        top: &StackItem,
        owners: Vec<tricerules_card_model::ProgramOwnerId>,
        previous: crate::state::EffectResult,
    ) {
        let run = engine
            .begin_program_run(top, owners)
            .expect("program starts");
        let mut events = Vec::new();
        assert!(matches!(
            engine
                .run_program_with_previous(top, run, previous, &mut events)
                .expect("program runs to first offer"),
            ResolutionProgress::Parked
        ));
        engine
            .publish_resolution_prompt_ids(&mut events)
            .expect("first offer gets prompt ID");
        assert!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .resolution_prompt_id
                .get()
                != 0
        );
    }

    fn nested_answer(
        engine: &mut GameEngine,
        decision: tricerules_proto::ruled::v1::ResolutionChoiceDecision,
        branch: u32,
        chosen: Vec<ObjectId>,
    ) {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("resolution choice pending");
        let actor = pending.deciding_player;
        let prompt_id = pending.resolution_prompt_id.get();
        let answer = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice({
                let mut answer = rv1::SubmitResolutionChoice {
                    resolution_prompt_id: prompt_id,
                    decision: decision as i32,
                    selected_branch_index: branch,
                    chosen_object_ids: chosen,
                    ..Default::default()
                };
                if decision == tricerules_proto::ruled::v1::ResolutionChoiceDecision::PayMana {
                    let pending_payment = pending
                        .continuation
                        .mana_payment()
                        .expect("the paid Choose branch owns a pending mana payment");
                    assert!(pending_payment.mana_cost.pips.is_empty());
                    assert_eq!(pending_payment.generic_mana_cost, 1);
                    let source_object_id = pending.presentation.source_object_id;
                    answer.payment = Some(rv1::PaymentSelection {
                        expected_state_revision: engine.state.command_index,
                        source: Some(rv1::CostObjectRef {
                            object_id: source_object_id,
                            zone_change_generation: engine
                                .state
                                .zone_change_generation
                                .get(&source_object_id)
                                .copied()
                                .unwrap_or(0),
                        }),
                        mana: Some(rv1::PaymentMana {
                            c: pending_payment.generic_mana_cost,
                            ..Default::default()
                        }),
                        ..Default::default()
                    });
                }
                answer
            })),
        };
        engine
            .apply_scenario_command(actor, &answer)
            .expect("resolution choice accepted");
    }

    fn nested_select_objects(engine: &mut GameEngine, chosen: Vec<ObjectId>) {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("object choice pending");
        let actor = pending.deciding_player;
        let prompt_id = pending.resolution_prompt_id.get();
        let answer = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: prompt_id,
                    chosen_object_ids: chosen,
                    ..Default::default()
                },
            )),
        };
        engine
            .apply_scenario_command(actor, &answer)
            .expect("object choice accepted");
    }

    fn parked_program(
        engine: &GameEngine,
    ) -> (
        &crate::state::PendingResolution,
        &crate::state::ParkedStackResolution,
        &ProgramRunState,
    ) {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("resolution pending");
        let stack = match &pending.continuation {
            ResolutionContinuation::ProgramChoose { stack, .. }
            | ResolutionContinuation::PermanentChoice { stack, .. }
            | ResolutionContinuation::HandChoice { stack, .. }
            | ResolutionContinuation::SearchLibrary { stack, .. } => stack,
            other => panic!("unexpected continuation: {other:?}"),
        };
        let program = stack.program.as_ref().expect("parked program state");
        (pending, stack, program)
    }

    fn nested_slot_id(
        engine: &GameEngine,
        owner: &tricerules_card_model::ProgramOwnerId,
        name: &str,
    ) -> ResultSlotId {
        engine
            .registry
            .compiled_program_for_owner(owner)
            .expect("compiled program exists")
            .slots
            .iter()
            .find(|slot| slot.name == name)
            .expect("named result slot")
            .id
    }

    fn nested_slot(run: &ProgramRunState, slot: ResultSlotId) -> Option<&ProgramValue> {
        run.slots.get(slot.0 as usize).and_then(Option::as_ref)
    }

    const NESTED_IF: &str = r#"(id: "nested_choose_if_fixture", name: "Nested Choose If Fixture",
 face_id: "nested_choose_if_fixture", mana_cost: "{0}", types: ["Instant"],
 spell_program: Some((body: (steps: [
  (id: "route", bind: [], input: None, kind: Choose(choice: "route", chooser: Controller,
   optional: true, branches: [(branch_id: "branch", presentation: Fallback, cost: None,
    requirement: Always, body: (steps: [(id: "inside_branch", bind: [], input: None,
     kind: If(condition: CardsInHand(players: Relative(Controller), min: Some(1), max: None),
      yes: (steps: [(id: "branch_yes", bind: [], input: None,
       kind: Perform(Draw(who: Controller, count: 1)))], yields: []),
      no: (steps: [(id: "branch_no", bind: [], input: None,
       kind: Perform(Draw(who: Controller, count: 2)))], yields: [])))], yields: []))],
   otherwise: (steps: [(id: "inside_otherwise", bind: [], input: None,
    kind: If(condition: CardsInHand(players: Relative(Controller), min: Some(99), max: None),
     yes: (steps: [(id: "otherwise_yes", bind: [], input: None,
      kind: Perform(Draw(who: Controller, count: 2)))], yields: []),
     no: (steps: [(id: "otherwise_no", bind: [], input: None,
      kind: Perform(Draw(who: Controller, count: 3)))], yields: [])))], yields: []))),
  (id: "tail", bind: [], input: None, kind: Perform(Draw(who: Controller, count: 1)))
 ], yields: []), target_bindings: [])))"#;

    #[test]
    fn nested_choose_executes_if_in_selected_and_otherwise_then_runs_one_parent_tail() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;
        for (decision, branch, expected_draws) in
            [(Decision::SelectBranch, 0, 2), (Decision::Decline, 0, 4)]
        {
            let (mut engine, top, owners, _) = nested_choose_fixture(
                NESTED_IF,
                "nested_choose_if_fixture",
                91_221 + u64::from(expected_draws),
                0,
                &[],
            );
            let before = engine.state.turn_history.current.player(0).cards_drawn;
            nested_start(
                &mut engine,
                &top,
                owners,
                crate::state::EffectResult::default(),
            );
            nested_answer(&mut engine, decision, branch, Vec::new());
            assert_eq!(
                engine.state.turn_history.current.player(0).cards_drawn,
                before + expected_draws
            );
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.state.stack.is_empty());
            assert_eq!(engine.state.players[0].graveyard.last(), Some(&top.id));
        }
    }

    const NESTED_YIELD: &str = r#"(id: "nested_choose_yield_fixture", name: "Nested Choose Yield Fixture",
 face_id: "nested_choose_yield_fixture", mana_cost: "{0}", types: ["Instant"],
 spell_program: Some((body: (steps: [
  (id: "draw_before", bind: ["drawn"], input: None, kind: Perform(Draw(who: Controller, count: 1))),
  (id: "pick", bind: ["picked"], input: None, kind: Choose(choice: "pick", chooser: Controller,
   optional: true, branches: [(branch_id: "branch", presentation: Fallback, cost: None,
    requirement: Always, body: (steps: [(id: "branch_pick", bind: ["branch_pick"], input: None,
     kind: Perform(ChoosePermanents(chooser: Controller, filter: (kind: AnyPermanent,
      permanent_types: [Land]), min: 1, max: 1)))], yields: ["branch_pick"]))],
   otherwise: (steps: [(id: "otherwise_pick", bind: ["otherwise_pick"], input: None,
    kind: Perform(ChoosePermanents(chooser: Controller, filter: (kind: AnyPermanent,
     permanent_types: [Land]), min: 1, max: 1)))], yields: ["otherwise_pick"]))),
  (id: "untap", bind: [], input: Some("picked"), kind: Perform(UntapChosenPermanents)),
  (id: "pause_after_yield", bind: [], input: None, kind: Choose(choice: "after", chooser: Controller,
   optional: false, branches: [(branch_id: "continue", presentation: Fallback, cost: None,
    requirement: Always, body: (steps: [], yields: []))], otherwise: (steps: [], yields: [])))
 ], yields: []), target_bindings: [])))"#;

    #[test]
    fn nested_choose_yields_nonempty_permanent_selection_to_parent_and_retains_sibling_slot() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;
        for (decision, branch, child_name, arm) in [
            (
                Decision::SelectBranch,
                0,
                "branch_pick",
                ProgramArm::ChooseBranch(tricerules_card_model::ChoiceId::new("branch").unwrap()),
            ),
            (
                Decision::Decline,
                0,
                "otherwise_pick",
                ProgramArm::ChooseOtherwise,
            ),
        ] {
            let (mut engine, top, owners, islands) = nested_choose_fixture(
                NESTED_YIELD,
                "nested_choose_yield_fixture",
                91_230 + branch as u64,
                2,
                &[],
            );
            let before = engine.state.turn_history.current.player(0).cards_drawn;
            let child_slot = nested_slot_id(&engine, &owners[0], child_name);
            let parent_slot = nested_slot_id(&engine, &owners[0], "picked");
            let draw_slot = nested_slot_id(&engine, &owners[0], "drawn");
            nested_start(
                &mut engine,
                &top,
                owners,
                crate::state::EffectResult::default(),
            );
            nested_answer(&mut engine, decision, branch, Vec::new());
            let (pending, _, run) = parked_program(&engine);
            assert!(matches!(
                &pending.continuation,
                ResolutionContinuation::PermanentChoice { .. }
            ));
            assert_eq!(run.in_flight, Some(vec![child_slot]));
            assert!(
                nested_slot(run, parent_slot).is_none(),
                "parent output is not published before child commit"
            );
            assert!(run
                .frames
                .iter()
                .flat_map(|frame| &frame.path)
                .any(|path| path.arm == arm));
            assert!(
                matches!(nested_slot(run, draw_slot), Some(ProgramValue::DrawReceipt(cards)) if cards.len() == 1)
            );
            assert_eq!(pending.presentation.candidates.len(), 2);
            let chosen = islands[0];
            nested_select_objects(&mut engine, vec![chosen]);
            let (_, _, run) = parked_program(&engine);
            assert!(
                matches!(nested_slot(run, parent_slot), Some(ProgramValue::PermanentSelection(cards)) if cards.len() == 1 && cards[0].object_id == chosen)
            );
            nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
            assert_eq!(
                engine.state.turn_history.current.player(0).cards_drawn,
                before + 1
            );
            assert!(!engine.state.objects[&chosen].tapped);
            assert!(engine.state.objects[&islands[1]].tapped);
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.state.stack.is_empty());
        }
    }

    const MODAL_PARKING: &str = r#"(id: "nested_modal_parking_fixture", name: "Nested Modal Parking Fixture",
 face_id: "nested_modal_parking_fixture", mana_cost: "{0}", types: ["Instant"],
 modal_spell: (min_modes: 2, max_modes: 2, modes: [
  (mode_id: "first", presentation: Fallback, effects: [], program: Some((body: (steps: [
   (id: "before", bind: ["before"], input: None, kind: Perform(Draw(who: Controller, count: 1))),
   (id: "pay_and_select", bind: [], input: None, kind: Choose(choice: "pay", chooser: Controller,
    optional: false, branches: [(branch_id: "pay", presentation: Fallback, cost: Mana("{1}"),
     requirement: Always, body: (steps: [
      (id: "select", bind: ["selected"], input: None,
       kind: Perform(SelectHandCards(count: 1, ordered: true, prompt: "Choose the card to put back."))),
      (id: "put_back", bind: [], input: Some("selected"), kind: Perform(PutSelectedHandCardsOnLibraryTop)),
      (id: "pause", bind: [], input: None, kind: Choose(choice: "after_put_back", chooser: Controller,
       optional: false, branches: [(branch_id: "continue", presentation: Fallback, cost: None,
        requirement: Always, body: (steps: [], yields: []))], otherwise: (steps: [], yields: [])))
     ], yields: []))], otherwise: (steps: [], yields: [])))
  ], yields: []), target_bindings: []))),
  (mode_id: "second", presentation: Fallback, effects: [], program: Some((body: (steps: [
   (id: "after", bind: ["after"], input: None, kind: Perform(Draw(who: Controller, count: 1)))
  ], yields: []), target_bindings: [])))
 ]))"#;

    const RETAINED_SEARCH_HAND_PARKING: &str = r#"(id: "nested_retained_search_fixture",
 name: "Nested Retained Search Fixture", face_id: "nested_retained_search_fixture",
 mana_cost: "{0}", types: ["Instant"], spell_program: Some((body: (steps: [
  (id: "before", bind: ["drawn"], input: None,
   kind: Perform(Draw(who: Controller, count: 1))),
  (id: "search", bind: ["found"], input: None, kind: Perform(SearchLibrary(
   who: Controller, optional: false, count: 1, count_by_cast_cost: None, filter: None,
   selection_constraint: None, slots: [], zones: Fixed([Library]), destination: KeepInLibrary,
   conditional_destination: None, shuffle: false, reveal: false, result_id: None,
   selection_bounds: Some((min: 1, max: 1)),
  ))),
  (id: "select", bind: ["selected"], input: None,
   kind: Perform(SelectHandCards(count: 1, ordered: true,
    prompt: "Choose a card to retain in hand.")))
 ], yields: []), target_bindings: [])))"#;

    #[test]
    fn modal_owner_receipt_slots_survive_choose_payment_and_hand_parking_before_next_mode() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;
        let (mut engine, top, owners, islands) = nested_choose_fixture(
            MODAL_PARKING,
            "nested_modal_parking_fixture",
            91_241,
            1,
            &["first", "second"],
        );
        assert_eq!(owners.len(), 2);
        let before_slot = nested_slot_id(&engine, &owners[0], "before");
        let selected_slot = nested_slot_id(&engine, &owners[0], "selected");
        let seed_object = islands[0];
        let seeded = crate::state::TriggerObjectRef {
            object_id: seed_object,
            zone_change_generation: engine.state.zone_change_generation[&seed_object],
            controller_at_event: 0,
        };
        let mut previous = crate::state::EffectResult::default();
        previous.produced_objects.push(seeded);
        engine.state.players[0].mana_pool.colorless = 1;
        let before_draws = engine.state.turn_history.current.player(0).cards_drawn;
        nested_start(&mut engine, &top, owners.clone(), previous);
        let (pending, stack, run) = parked_program(&engine);
        assert_eq!(run.owner, owners[0]);
        assert_eq!(run.pending_owners, owners[1..]);
        assert_eq!(stack.previous_result.produced_objects, vec![seeded]);
        let draw_ref = match nested_slot(run, before_slot) {
            Some(ProgramValue::DrawReceipt(cards)) if cards.len() == 1 => cards[0],
            other => panic!("first modal owner must retain its draw receipt: {other:?}"),
        };
        let drawn_id = draw_ref.object_id;
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            before_draws + 1
        );
        let _ = pending;

        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        assert!(matches!(
            &engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .continuation,
            ResolutionContinuation::ProgramChoose {
                choose: crate::state::PendingProgramChoose {
                    stage: crate::state::PendingProgramChooseStage::PayingMana {
                        selected_branch: 0,
                        ..
                    },
                    ..
                },
                ..
            }
        ));
        let (_, stack, run) = parked_program(&engine);
        assert_eq!(run.owner, owners[0]);
        assert_eq!(run.pending_owners, owners[1..]);
        assert_eq!(stack.previous_result.produced_objects, vec![seeded]);
        assert!(
            matches!(nested_slot(run, before_slot), Some(ProgramValue::DrawReceipt(cards)) if cards[0].object_id == drawn_id)
        );

        nested_answer(&mut engine, Decision::PayMana, 0, Vec::new());
        let (pending, stack, run) = parked_program(&engine);
        assert!(matches!(
            &pending.continuation,
            ResolutionContinuation::HandChoice { .. }
        ));
        assert_eq!(run.owner, owners[0]);
        assert_eq!(run.pending_owners, owners[1..]);
        assert_eq!(stack.previous_result.produced_objects, vec![seeded]);
        assert_eq!(run.in_flight, Some(vec![selected_slot]));
        assert!(
            nested_slot(run, selected_slot).is_none(),
            "the pending hand input is distinct from the saved run-level result"
        );
        assert!(
            matches!(nested_slot(run, before_slot), Some(ProgramValue::DrawReceipt(cards)) if cards[0].object_id == drawn_id)
        );
        assert!(pending.presentation.candidates.contains(&drawn_id));

        nested_select_objects(&mut engine, vec![drawn_id]);
        let (pending, stack, run) = parked_program(&engine);
        assert_eq!(run.owner, owners[0]);
        assert_eq!(run.pending_owners, owners[1..]);
        assert_eq!(stack.previous_result.produced_objects, vec![seeded]);
        assert!(matches!(
            &pending.continuation,
            ResolutionContinuation::ProgramChoose {
                choose: crate::state::PendingProgramChoose {
                    stage: crate::state::PendingProgramChooseStage::Selecting,
                    ..
                },
                ..
            }
        ));
        assert!(
            matches!(nested_slot(run, before_slot), Some(ProgramValue::DrawReceipt(cards)) if cards[0].object_id == drawn_id)
        );
        assert!(
            matches!(nested_slot(run, selected_slot), Some(ProgramValue::HandSelection(cards)) if cards.len() == 1 && cards[0].object_id == drawn_id)
        );
        assert_eq!(engine.state.players[0].library.front(), Some(&drawn_id));
        let at_library_generation = engine.state.zone_change_generation[&drawn_id];

        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            before_draws + 2
        );
        assert_eq!(
            engine.state.zone_change_generation[&drawn_id],
            at_library_generation + 1
        );
        assert!(engine.state.players[0].hand.contains(&drawn_id));
        assert_eq!(engine.state.players[0].graveyard.last(), Some(&top.id));
    }

    #[test]
    fn retained_search_park_and_resume_preserves_run_context_and_typed_output() {
        let (mut engine, top, owners, _) = nested_choose_fixture(
            RETAINED_SEARCH_HAND_PARKING,
            "nested_retained_search_fixture",
            91_242,
            0,
            &[],
        );
        let seeded_object = *engine.state.players[0]
            .library
            .front()
            .expect("fixture has a retained-search candidate");
        let seeded_generation = engine
            .state
            .zone_change_generation
            .get(&seeded_object)
            .copied()
            .unwrap_or(0);
        let seeded = crate::state::TriggerObjectRef {
            object_id: seeded_object,
            zone_change_generation: seeded_generation,
            controller_at_event: 0,
        };
        let mut previous = crate::state::EffectResult::default();
        previous.produced_objects.push(seeded);
        let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
        let drawn_slot = nested_slot_id(&engine, &owners[0], "drawn");
        let found_slot = nested_slot_id(&engine, &owners[0], "found");
        let selected_slot = nested_slot_id(&engine, &owners[0], "selected");

        nested_start(&mut engine, &top, owners, previous);
        let (pending, stack, run) = parked_program(&engine);
        assert!(matches!(
            &pending.continuation,
            ResolutionContinuation::SearchLibrary { .. }
        ));
        assert_eq!(stack.previous_result.produced_objects, vec![seeded]);
        assert_eq!(run.in_flight, Some(vec![found_slot]));
        assert!(nested_slot(run, found_slot).is_none());
        assert!(
            matches!(nested_slot(run, drawn_slot), Some(ProgramValue::DrawReceipt(cards)) if cards.len() == 1)
        );
        let found_object = pending.presentation.candidates[0];

        nested_select_objects(&mut engine, vec![found_object]);
        let (pending, stack, run) = parked_program(&engine);
        assert!(matches!(
            &pending.continuation,
            ResolutionContinuation::HandChoice { .. }
        ));
        assert_eq!(stack.previous_result.produced_objects, vec![seeded]);
        assert_eq!(run.in_flight, Some(vec![selected_slot]));
        assert!(matches!(
            nested_slot(run, found_slot),
            Some(ProgramValue::FoundLibraryCohort(cards))
                if cards.len() == 1
                    && cards[0].object_id == found_object
                    && cards[0].zone == Zone::Library
        ));
        assert!(
            matches!(nested_slot(run, drawn_slot), Some(ProgramValue::DrawReceipt(cards)) if cards.len() == 1)
        );
        let selected_object = pending.presentation.candidates[0];

        nested_select_objects(&mut engine, vec![selected_object]);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            draws_before + 1
        );
    }
}
