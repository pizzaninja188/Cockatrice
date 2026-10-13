//! Runtime for the finite, registry-compiled spell-resolution program subset.
//!
//! The registry owns immutable instructions. Parked state contains only the active nested block
//! paths, cursors, typed values, and the result slots for one operation whose domain transaction
//! has not completed yet.

use super::*;
use crate::state::{ProgramArm, ProgramFrame, ProgramPathStep, ProgramRunState, ProgramValue};
use tricerules_card_model::primitives::{
    CompiledBlock, CompiledInstructionKind, CompiledProgram, ResultSlotId, ResultType,
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
    pub(in crate::engine) fn current_program_perform(
        &self,
        program_state: &ProgramRunState,
    ) -> Result<SpellEffectKind, EngineError> {
        let compiled = self
            .registry
            .compiled_program(&program_state.card_id, &program_state.face_id)
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
        self.run_program_with_previous_inner(top, program_state, previous_result, events)
            .map_err(|error| match error {
                // Once a compiled, bounded program starts, an Illegal result can only mean its
                // authored contract or committed internal transaction became inconsistent.
                // Public actor, generation, and offered-choice validation runs before this point.
                EngineError::Illegal(reason) => EngineError::ProgramInvariant(reason),
                other => other,
            })
    }

    fn run_program_with_previous_inner(
        &mut self,
        top: &StackItem,
        mut program_state: ProgramRunState,
        previous_result: EffectResult,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<ResolutionProgress, EngineError> {
        if self.state.is_terminal() {
            return Ok(ResolutionProgress::GameEnded);
        }
        if program_state.card_id != top.card_id {
            return Err(EngineError::ProgramInvariant(
                "resolution program card identity changed",
            ));
        }

        // Copy the shared registry reference before entering the mutable execution loop. The
        // compiled tree remains borrowed from this immutable process-wide registry throughout;
        // only the currently executing leaf operation is cloned.
        let registry = self.registry;
        let compiled = registry
            .compiled_program(&program_state.card_id, &program_state.face_id)
            .ok_or(EngineError::ProgramInvariant(
                "compiled resolution program disappeared",
            ))?;
        if program_state.slots.len() != compiled.slots.len() || program_state.frames.is_empty() {
            return Err(EngineError::ProgramInvariant(
                "invalid resolution program cursor",
            ));
        }

        if let Some(outputs) = program_state.in_flight.take() {
            #[cfg(test)]
            trip_program_failpoint(ProgramFailpoint::DomainCommitted)?;
            capture_program_outputs(compiled, &mut program_state, &outputs, &previous_result)?;
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

        loop {
            if self.state.is_terminal() {
                return Ok(ResolutionProgress::GameEnded);
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

                let label = registry
                    .get(&top.card_id)
                    .and_then(|definition| definition.face(top.face_index))
                    .map(|face| face.name.to_string())
                    .unwrap_or_else(|| top.card_id.clone());
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
                    let selected_arm = if self
                        .condition_holds(&condition, ConditionContext::for_stack_item(top))
                    {
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
                CompiledInstructionKind::Perform(effect) => {
                    let effect = effect.as_ref().clone();
                    let outputs = instruction.outputs.clone();
                    let input = instruction.input;
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
                    let mut cx = EffectCx {
                        engine: self,
                        events,
                        targets: &[],
                        targets_by_role: &[],
                        target_damage: &[],
                        target_group_indices: &[],
                        top,
                        controller: top.controller,
                        affected_player: top.controller,
                        spell_label: &label,
                        previous_effect_result: &previous,
                        effect_result: &mut result,
                        effect_index: instruction_index as u32,
                        program_state: Some(program_state.clone()),
                    };
                    let outcome = if matches!(
                        &effect,
                        SpellEffectKind::SelectHandCards { .. }
                            | SpellEffectKind::PutSelectedHandCardsOnLibraryTop
                    ) {
                        run_program_only_leaf(&mut cx, effect)?
                    } else {
                        dispatch::execute_effect(&mut cx, effect)?
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
                    if outcome != EffectOutcome::Continue {
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
                        &result,
                        events,
                    )? {
                        return Ok(ResolutionProgress::Parked);
                    }
                }
            }
        }
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

fn block_at_path<'a>(
    program: &'a CompiledProgram,
    path: &[ProgramPathStep],
) -> Result<&'a CompiledBlock, EngineError> {
    let mut block = &program.body;
    for step in path {
        let instruction = block.steps.get(step.instruction_index as usize).ok_or(
            EngineError::ProgramInvariant("resolution program frame path is invalid"),
        )?;
        let CompiledInstructionKind::If { yes, no, .. } = &instruction.kind else {
            return Err(EngineError::ProgramInvariant(
                "resolution program frame is not a branch",
            ));
        };
        block = match step.arm {
            ProgramArm::Yes => yes,
            ProgramArm::No => no,
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
            ResultType::DiscardReceipt { .. } => ProgramValue::DiscardReceipt(result.cards.clone()),
            ResultType::PermanentSelection { .. } => {
                ProgramValue::PermanentSelection(result.produced_objects.clone())
            }
            ResultType::HandSelection { .. } => {
                ProgramValue::HandSelection(result.selected_hand_cards.clone())
            }
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
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .expect("first player passes");
        assert_eq!(engine.state.priority_player_id(), 1);
        engine
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
                engine.apply_command(1, &pass_priority())
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
                .apply_command(1, &pass_priority())
                .expect("failed command can be retried");
            let accepted = control
                .apply_command(1, &pass_priority())
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
                .apply_command(1, &pass_priority())
                .expect("Brainstorm draws three and parks on hand choice");
            let mut control = engine.clone();
            let answer = choose_first_two_offered_hand_cards(&engine);
            let before = command_snapshot(&engine);

            let failure = {
                let _guard = ProgramFailpointGuard::arm(point);
                engine.apply_command(0, &answer)
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
                .apply_command(0, &answer)
                .expect("the same hand-choice answer can be retried");
            let accepted = control
                .apply_command(0, &answer)
                .expect("control hand-choice answer succeeds");
            assert_eq!(
                retried, accepted,
                "retry after {point:?} must match the no-fault batch"
            );
            assert!(command_snapshot(&engine) == command_snapshot(&control));
        }
    }
}
