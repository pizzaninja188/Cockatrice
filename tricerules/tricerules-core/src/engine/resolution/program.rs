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
    Cardinality, ChooseDeparturePolicy, CompiledBlock, CompiledChooseBranch,
    CompiledChooseRequirement, CompiledInstructionKind, CompiledProgram, CompiledProgramCondition,
    CompiledProgramResultCondition, HandCardAction, HandCardChooser, HandChoiceVisibility,
    PermanentTypeFilter, ProgramRequiredInput, ResolutionCost, ResultSlotId, ResultType,
    SpellEffectKind, TargetFilter, TargetKind,
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
mod source_sacrifice_receipt_tests {
    use super::*;
    use crate::state::{CardResultCohort, CardResultEntry, CardResultOutputVisibility};
    use tricerules_card_model::primitives::{CardResultAction, CardTypeFilter};

    fn entry(
        action: CardResultAction,
        affected_player: PlayerId,
        object_id: ObjectId,
        input_zone: Zone,
        input_generation: u64,
    ) -> CardResultEntry {
        CardResultEntry {
            action,
            affected_player,
            object_id,
            zone_change_generation: input_generation + 1,
            matched_card_types: Vec::<CardTypeFilter>::new(),
            provenance: crate::state::CardResultProvenance::CommittedMovement {
                input_zone,
                input_generation,
                actual_output_zone: Zone::Graveyard,
                output_visibility: CardResultOutputVisibility::FaceUpPublic,
            },
        }
    }

    fn cohort(entry: CardResultEntry) -> CardResultCohort {
        CardResultCohort {
            cards: vec![entry],
            program_discard_result: None,
        }
    }

    #[test]
    fn source_sacrifice_program_count_uses_effective_pre_move_basic_land_type() {
        use crate::state::{EffectResult, ProgramRunState};
        use tricerules_card_model::primitives::{
            CardTypeFilter, CompiledProgramResultCondition, ResultSlotId,
        };

        let registry = tricerules_cards::registry::global();
        let decks = Some(vec![
            vec!["standstill".to_string(); 24],
            vec!["forest".to_string(); 24],
        ]);
        let mut engine = GameEngine::new(registry, 701_211, &[10, 20], 20, decks, true)
            .expect("Standstill and Forest registry fixture");
        let source = engine.state.players[0]
            .library
            .pop_front()
            .expect("Standstill source");
        engine.state.players[0].battlefield.push(source);
        let source_object = engine.state.objects.get_mut(&source).unwrap();
        source_object.zone = Zone::Battlefield;
        source_object.controller = 10;
        source_object.base_controller = 10;

        let forest = engine.state.players[1]
            .library
            .front()
            .copied()
            .expect("Forest copy source");
        let copied_values = engine
            .copiable_values_for(forest)
            .expect("Forest copiable values");
        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .copiable_values = Some(copied_values);
        let before = engine
            .characteristics(source)
            .expect("copied source characteristics");
        assert!(before.has_type("Land"));
        assert!(before.supertypes.iter().any(|value| value == "Basic"));
        assert!(!before.has_type("Enchantment"));
        assert!(registry
            .get("standstill")
            .unwrap()
            .primary_face()
            .matches_card_type(CardTypeFilter::Enchantment));

        let source_generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let top = StackItem {
            prototyped: false,
            mana_colors_spent_to_cast: Default::default(),
            id: 701_212,
            controller: 10,
            card_id: "standstill".into(),
            targets: Vec::new(),
            ability_text: Some("Whenever a player casts a spell".into()),
            source_permanent_id: Some(source),
            source_owner: Some(10),
            source_zone_change: source_generation,
            trigger_use_key: None,
            source_face_change: 0,
            ability_index: Some(0),
            activated_ability: None,
            activated_ability_occurrence: None,
            triggered_ability: None,
            is_triggered: true,
            is_copy: false,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            returned_attacker_assignment: None,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: None,
            cast_cost_receipts: Vec::new(),
            payment_result: Default::default(),
            search_results: Default::default(),
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: Default::default(),
        };
        let mut events = Vec::new();
        let mut result = EffectResult::default();
        let previous = EffectResult::default();
        let outcome = super::super::misc::sacrifice(
            &mut EffectCx {
                engine: &mut engine,
                events: &mut events,
                targets: &[],
                targets_by_role: &[],
                target_damage: &[],
                target_group_indices: &[],
                top: &top,
                controller: 10,
                affected_player: 20,
                spell_label: "Standstill",
                previous_effect_result: &previous,
                effect_result: &mut result,
                effect_index: 0,
                resume_previous_result: None,
                program_state: None,
            },
            SpellEffectKind::Sacrifice {
                subject: EffectSubject::Source,
            },
        )
        .expect("Standstill source sacrifice");
        assert_eq!(outcome, EffectOutcome::Continue);
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        assert_eq!(result.cards.len(), 1);

        let run = ProgramRunState {
            owner: tricerules_card_model::ProgramOwnerId::face(
                "standstill",
                tricerules_card_model::CardFaceId::new("standstill").unwrap(),
            ),
            pending_owners: Vec::new(),
            frames: Vec::new(),
            slots: vec![Some(ProgramValue::CardResultCohort(CardResultCohort {
                cards: result.cards.clone(),
                program_discard_result: None,
            }))],
            in_flight: None,
            in_flight_produced_object_provenance: None,
        };
        let count_matches = |card_type| {
            program_result_condition_holds(
                &engine,
                &top,
                &run,
                &CompiledProgramResultCondition::CardResultCount {
                    result: ResultSlotId(0),
                    card_type: Some(card_type),
                    min: Some(1),
                    max: Some(1),
                },
            )
            .expect("typed Program count condition")
        };
        assert!(count_matches(CardTypeFilter::BasicLand));
        assert!(!count_matches(CardTypeFilter::Enchantment));
    }

    #[test]
    fn source_sacrifice_projection_requires_exact_committed_input_facts_and_cardinality() {
        let expected = crate::state::TriggerObjectRef {
            object_id: 41,
            zone_change_generation: 7,
            controller_at_event: 0,
        };
        assert_eq!(
            source_sacrifice_receipt_from_cohort(
                &cohort(entry(
                    CardResultAction::Sacrifice,
                    0,
                    41,
                    Zone::Battlefield,
                    7
                )),
                41,
                7,
                0,
            )
            .unwrap(),
            Some(expected)
        );
        assert_eq!(
            source_sacrifice_receipt_from_cohort(&CardResultCohort::default(), 41, 7, 0).unwrap(),
            None,
            "an empty source-sacrifice result creates no reflexive trigger"
        );

        let mut action_facts = entry(CardResultAction::Sacrifice, 0, 41, Zone::Battlefield, 7);
        action_facts.provenance = crate::state::CardResultProvenance::ActionFacts;
        let invalid_single_entries = [
            action_facts,
            entry(CardResultAction::Destroy, 0, 41, Zone::Battlefield, 7),
            entry(CardResultAction::Sacrifice, 0, 41, Zone::Graveyard, 7),
            entry(CardResultAction::Sacrifice, 0, 41, Zone::Battlefield, 8),
            entry(CardResultAction::Sacrifice, 0, 42, Zone::Battlefield, 7),
            entry(CardResultAction::Sacrifice, 1, 41, Zone::Battlefield, 7),
        ];
        for invalid in invalid_single_entries {
            assert!(source_sacrifice_receipt_from_cohort(&cohort(invalid), 41, 7, 0).is_err());
        }

        let two_cards = CardResultCohort {
            cards: vec![
                entry(CardResultAction::Sacrifice, 0, 41, Zone::Battlefield, 7),
                entry(CardResultAction::Sacrifice, 0, 41, Zone::Battlefield, 7),
            ],
            program_discard_result: None,
        };
        assert!(
            source_sacrifice_receipt_from_cohort(&two_cards, 41, 7, 0).is_err(),
            "the 0..1 source-sacrifice result rejects multiple cards"
        );
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::engine) struct TargetHandDiscardProgramSignature {
    pub instruction_address: u32,
    pub target_group_index: u32,
    pub card_count: u32,
    pub cardinality: tricerules_card_model::primitives::Cardinality,
    pub result_slot: ResultSlotId,
}

impl GameEngine {
    /// Describe the exact compiled target-hand-discard operation while its card-choice
    /// transaction is still in flight. The immutable instruction, target binding, and typed
    /// result are one signature used by answer validation and every parked-continuation path.
    pub(in crate::engine) fn target_hand_discard_program_signature(
        &self,
        stack: &ParkedStackResolution,
    ) -> Option<TargetHandDiscardProgramSignature> {
        let program_state = stack.program.as_ref()?;
        let compiled = self
            .registry
            .compiled_program_for_owner(&program_state.owner)?;
        if compiled.owner != program_state.owner
            || compiled.context != tricerules_card_model::primitives::EffectContext::Spell
        {
            return None;
        }
        let frame = program_state.frames.last()?;
        let instruction_index = frame.next_instruction.checked_sub(1)? as usize;
        let block = block_at_path(compiled, &frame.path).ok()?;
        let instruction = block.steps.get(instruction_index)?;
        let CompiledInstructionKind::Perform(effect) = &instruction.kind else {
            return None;
        };
        let card_count = match effect.as_ref() {
            SpellEffectKind::ChooseHandCards {
                action: HandCardAction::Discard,
                count,
                target,
                chooser: HandCardChooser::AffectedPlayer,
                card_filter: None,
                optional: false,
                visibility: HandChoiceVisibility::PrivateLook,
            } if *count > 0
                && target
                    == &(TargetFilter {
                        kind: TargetKind::AnyPlayer,
                        ..TargetFilter::default()
                    }) =>
            {
                *count
            }
            _ => return None,
        };
        if instruction.input.is_some() {
            return None;
        }
        let bindings = compiled
            .target_bindings
            .iter()
            .filter(|binding| binding.instruction_address == instruction.instruction_address)
            .collect::<Vec<_>>();
        let [binding] = bindings.as_slice() else {
            return None;
        };
        if binding.role_index != 0 {
            return None;
        }
        let target_group_index = binding.target_group_index;
        let projected =
            program_target_context(&stack.item, compiled, instruction.instruction_address).ok()?;
        if !projected.has_bindings
            || projected.targets.len() != 1
            || projected.target_group_indices.as_slice() != [target_group_index]
        {
            return None;
        }

        let [output] = instruction.outputs.as_slice() else {
            return None;
        };
        let Some([in_flight]) = program_state.in_flight.as_deref() else {
            return None;
        };
        if output != in_flight {
            return None;
        }
        let slot = compiled.slots.get(output.0 as usize)?;
        let cardinality = match &slot.result_type {
            ResultType::TargetCardResultCohort {
                action: tricerules_card_model::primitives::CardResultAction::Discard,
                cardinality:
                    cardinality @ tricerules_card_model::primitives::Cardinality {
                        min: 0,
                        max: Some(max),
                    },
                target_group_index: result_group,
            } if *max == card_count && *result_group == target_group_index => *cardinality,
            _ => return None,
        };
        Some(TargetHandDiscardProgramSignature {
            instruction_address: instruction.instruction_address,
            target_group_index,
            card_count,
            cardinality,
            result_slot: *output,
        })
    }

    pub(in crate::engine) fn is_target_hand_discard_program(
        &self,
        stack: &ParkedStackResolution,
    ) -> bool {
        self.target_hand_discard_program_signature(stack).is_some()
    }

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
        validate_program_unit_inputs(self, top, compiled)?;
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
            in_flight_produced_object_provenance: None,
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
        validate_program_unit_inputs(self, top, active_compiled)?;
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
            capture_program_outputs_for_stack(
                self,
                top,
                active_compiled,
                &mut program_state,
                &outputs,
                operation_result,
            )?;
            program_state.in_flight_produced_object_provenance = None;
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
            validate_program_unit_inputs(self, top, compiled)?;
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
                    validate_program_unit_inputs(self, top, next)?;
                    program_state.pending_owners.remove(0);
                    program_state.owner = next_owner;
                    program_state.frames = vec![ProgramFrame {
                        path: Vec::new(),
                        next_instruction: 0,
                        yield_to: Vec::new(),
                    }];
                    program_state.slots = vec![None; next.slots.len()];
                    program_state.in_flight = None;
                    program_state.in_flight_produced_object_provenance = None;
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
                CompiledInstructionKind::ImportCastPayment => {
                    let [slot_id] = instruction.outputs.as_slice() else {
                        return Err(EngineError::ProgramInvariant(
                            "cast-payment import does not have exactly one output slot",
                        ));
                    };
                    let slot = compiled
                        .slot(*slot_id)
                        .ok_or(EngineError::ProgramInvariant(
                            "cast-payment import slot disappeared",
                        ))?;
                    if !matches!(
                        &slot.result_type,
                        ResultType::CastPaymentCohort {
                            action: tricerules_card_model::primitives::CardResultAction::Discard,
                            cardinality: tricerules_card_model::primitives::Cardinality {
                                min: 1,
                                max: Some(1),
                            },
                        }
                    ) {
                        return Err(EngineError::ProgramInvariant(
                            "cast-payment import has the wrong compiled result type",
                        ));
                    }
                    validate_program_cast_payment_import(top, compiled)?;
                    let destination = program_state.slots.get_mut(slot_id.0 as usize).ok_or(
                        EngineError::ProgramInvariant(
                            "cast-payment import slot is outside the run state",
                        ),
                    )?;
                    if destination.is_some() {
                        return Err(EngineError::ProgramInvariant(
                            "cast-payment import slot was already populated",
                        ));
                    }
                    *destination = Some(ProgramValue::CardResultCohort(top.payment_result.clone()));
                    program_state.frames[frame_index].next_instruction += 1;
                    if self.finish_program_instruction_boundary(
                        top,
                        &program_state,
                        &previous_result,
                        events,
                    )? {
                        return Ok(ResolutionProgress::Parked);
                    }
                }
                CompiledInstructionKind::If { condition, yes, no } => {
                    let condition = condition.clone();
                    let outputs = instruction.outputs.clone();
                    let parent_path = path;
                    let holds = match condition.as_ref() {
                        CompiledProgramCondition::Game(condition) => {
                            self.condition_holds(condition, ConditionContext::for_stack_item(top))
                        }
                        CompiledProgramCondition::Result(condition) => {
                            program_result_condition_holds(self, top, &program_state, condition)?
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
                    chooser,
                    optional,
                    departure_policy,
                    branches,
                    otherwise,
                    ..
                } => {
                    let occurrence = crate::state::ProgramChooseOccurrence {
                        parent_path: path.clone(),
                        instruction_index: instruction_index as u32,
                        choice_id: choice.clone(),
                    };
                    let chooser_player = match chooser {
                        tricerules_card_model::primitives::PlayerRecipient::Controller => {
                            top.controller
                        }
                        tricerules_card_model::primitives::PlayerRecipient::SourceController => {
                            super::source_controller(self, top).ok_or(EngineError::Illegal(
                                "source-controller Choose has no current or last-known source controller",
                            ))?
                        }
                        tricerules_card_model::primitives::PlayerRecipient::AffectedPlayer => {
                            top.trigger_context.affected_player.ok_or(
                                EngineError::ProgramInvariant(
                                    "compiled AffectedPlayer Choose lacks its captured trigger player",
                                ),
                            )?
                        }
                        tricerules_card_model::primitives::PlayerRecipient::TargetedPlayer {
                            group_index,
                            kind: TargetKind::OpponentPlayer,
                        } => {
                            let owner_targets = program_owner_target_records(top, compiled)?;
                            let mut group_targets = owner_targets
                                .iter()
                                .filter(|target| target.group_index == *group_index);
                            let Some(target) = group_targets.next() else {
                                return Err(EngineError::ProgramInvariant(
                                    "TargetedPlayer Choose lacks its captured owner-local target group",
                                ));
                            };
                            let chooser_player = target.object_id as PlayerId;
                            if group_targets.next().is_some()
                                || self.state.player_idx(chooser_player).is_none()
                            {
                                return Err(EngineError::ProgramInvariant(
                                    "TargetedPlayer Choose requires exactly one captured player target",
                                ));
                            }
                            chooser_player
                        }
                        _ => {
                            return Err(EngineError::ProgramInvariant(
                                "compiled Choose has an unsupported chooser",
                            ));
                        }
                    };
                    let choose_controller_live = self
                        .state
                        .player_idx(chooser_player)
                        .is_some_and(|idx| !self.state.players[idx].has_lost);
                    if *departure_policy == ChooseDeparturePolicy::OptionalCostFallsThrough
                        && !choose_controller_live
                    {
                        program_state.frames[frame_index].next_instruction += 1;
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
                        continue;
                    }
                    let live_branches = branches
                        .iter()
                        .enumerate()
                        .filter(|(_, branch)| {
                            (choose_controller_live || branch.cost == ResolutionCost::None)
                                && self.program_choose_cost_is_selectable(
                                    top,
                                    chooser_player,
                                    &branch.cost,
                                )
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
                    if !*optional && live_branches.len() == 1 {
                        let branch_index = live_branches[0];
                        if let Some(branch) = branches
                            .get(branch_index)
                            .filter(|branch| branch.cost == ResolutionCost::None)
                        {
                            events.push(ev_log(format!(
                                "P{} chooses: {}.",
                                chooser_player,
                                branch.fallback_label()
                            )));
                            let mut child_path = path;
                            child_path.push(ProgramPathStep {
                                instruction_index: instruction_index as u32,
                                arm: ProgramArm::ChooseBranch(branch.branch_id.clone()),
                            });
                            program_state.frames.push(ProgramFrame {
                                path: child_path,
                                next_instruction: 0,
                                yield_to: instruction.outputs.clone(),
                            });
                            continue;
                        }
                    }
                    let (deciding_player, choose_stage, delegate_selected, prompt) =
                        if choose_controller_live {
                            (
                                chooser_player,
                                crate::state::PendingProgramChooseStage::Selecting,
                                false,
                                prompt,
                            )
                        } else {
                            let candidates = super::resolution_choice_delegate_candidates(
                                &self.state,
                                top.controller,
                                chooser_player,
                            );
                            match candidates.as_slice() {
                                [] => {
                                    events.push(ev_log(
                                        "No live player can make the departed chooser's resolution choice; Otherwise resolves.".into(),
                                    ));
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
                                    continue;
                                }
                                [delegate] => (
                                    *delegate,
                                    crate::state::PendingProgramChooseStage::Selecting,
                                    true,
                                    prompt,
                                ),
                                _ => {
                                    let controller_is_live = self
                                        .state
                                        .player_idx(top.controller)
                                        .is_some_and(|idx| !self.state.players[idx].has_lost);
                                    let selector = if controller_is_live {
                                        Some(top.controller)
                                    } else {
                                        self.state
                                            .player_idx(top.controller)
                                            .and_then(|idx| self.state.next_in_game_player_idx(idx))
                                            .map(|idx| self.state.players[idx].id)
                                            .or_else(|| {
                                                self.state
                                                    .players
                                                    .iter()
                                                    .find(|player| !player.has_lost)
                                                    .map(|player| player.id)
                                            })
                                    };
                                    let Some(selector) = selector else {
                                        events.push(ev_log(
                                            "No live player can select a delegate for the departed resolution chooser; Otherwise resolves.".into(),
                                        ));
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
                                        continue;
                                    };
                                    (
                                        selector,
                                        crate::state::PendingProgramChooseStage::ChoosingDelegate {
                                            candidates: candidates
                                                .iter()
                                                .copied()
                                                .map(Some)
                                                .collect(),
                                        },
                                        false,
                                        format!(
                                            "P{selector}: choose a player to make P{chooser_player}'s resolution choice."
                                        ),
                                    )
                                }
                            }
                        };
                    self.state.pending_resolution = Some(crate::state::PendingResolution {
                        resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
                        resolution_prompt_semantics: None,
                        deciding_player,
                        presentation: crate::state::PendingResolutionPresentation {
                            source_object_id: top.id,
                            candidates: Vec::new(),
                            min: u32::from(
                                matches!(
                                    &choose_stage,
                                    crate::state::PendingProgramChooseStage::ChoosingDelegate { .. }
                                ) || !*optional,
                            ),
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
                                original_chooser: chooser_player,
                                optional: *optional,
                                departure_policy: *departure_policy,
                                delegate_selected,
                                stage: choose_stage,
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
                    program_state.in_flight_produced_object_provenance = None;

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
                            ProgramValue::ManifestedPermanent(Some(object)) => {
                                previous.produced_objects.push(*object);
                            }
                            ProgramValue::ManifestedPermanent(None) => {}
                            ProgramValue::CounteredAbilitySource(Some(object)) => {
                                previous.produced_objects.push(*object);
                            }
                            ProgramValue::CounteredAbilitySource(None) => {}
                            ProgramValue::HandSelection(objects) => {
                                previous.selected_hand_cards = objects.clone();
                            }
                            ProgramValue::CardResultCohort(cohort) => {
                                previous.cards = cohort.cards.clone();
                            }
                            ProgramValue::TokenCopyBatch(objects) => {
                                previous.produced_objects = objects.clone();
                                previous.produced_object_provenance = Some(
                                    crate::state::ProducedObjectProvenance::TokenCreationBatch,
                                );
                            }
                            ProgramValue::CreatedTokenBatch(objects) => {
                                previous.produced_objects = objects.clone();
                            }
                            ProgramValue::TriggerObjectLinkedExileReceipt(Some(object)) => {
                                previous.produced_objects.push(*object);
                            }
                            ProgramValue::TriggerObjectLinkedExileReceipt(None) => {}
                            ProgramValue::ExiledObject(Some(object)) => {
                                previous.produced_objects.push(*object);
                            }
                            ProgramValue::ExiledObject(None) => {}
                            value @ (ProgramValue::FoundLibraryCohort(_)
                            | ProgramValue::RevealedLibraryCohort(_)
                            | ProgramValue::CardPartition(_)) => {
                                previous.program_value = Some(value.clone());
                            }
                            _ => {
                                return Err(EngineError::ProgramInvariant(
                                    "resolution program input has the wrong type",
                                ));
                            }
                        }
                    }
                    if is_program_source_sacrifice_reflexive(&effect) {
                        if let Some(receipt) = program_source_sacrifice_reflexive_receipt(
                            compiled,
                            &program_state,
                            input,
                            top,
                        )? {
                            previous.sacrificed_objects.push(receipt);
                        }
                    }
                    let mut result = EffectResult::default();
                    let label = registry
                        .get(&top.card_id)
                        .and_then(|definition| definition.face(top.face_index))
                        .map(|face| face.name.to_string())
                        .unwrap_or_else(|| top.card_id.clone());
                    if is_kyoshi_equipment_receipt_attach(compiled, instruction, &effect) {
                        if let Some(source_id) = top.source_permanent_id {
                            if let Some(equipment) = super::current_equipment_receipt_source(
                                self,
                                source_id,
                                top.source_zone_change,
                            ) {
                                let candidates = super::current_equipment_receipt_candidates(
                                    self,
                                    equipment,
                                    &previous.produced_objects,
                                );
                                match candidates.as_slice() {
                                    [] => {}
                                    [(candidate, generation)] => {
                                        let _ = super::misc::attach_equipment_ids(
                                            self,
                                            equipment.object_id,
                                            *candidate,
                                            &label,
                                            events,
                                        );
                                        debug_assert_eq!(
                                            self.state
                                                .zone_change_generation
                                                .get(candidate)
                                                .copied(),
                                            Some(*generation),
                                            "automatic receipt attachment keeps its exact candidate generation"
                                        );
                                    }
                                    _ => {
                                        let chooser = equipment.controller_at_event;
                                        let chooser_index = self
                                            .state
                                            .player_idx(chooser)
                                            .ok_or(EngineError::ProgramInvariant(
                                                "current Equipment controller is missing from turn order",
                                            ))?;
                                        let prompt = "Choose one of the Ally creatures created by Kyoshi Battle Fan.".to_string();
                                        let mut stack = ParkedStackResolution::new(top.clone())
                                            .with_previous_result(previous_result.clone());
                                        stack.program = Some(program_state.clone());
                                        self.state.pending_resolution =
                                            Some(crate::state::PendingResolution {
                                                resolution_prompt_id: crate::state::ResolutionPromptId::UNASSIGNED,
                                                resolution_prompt_semantics: None,
                                                deciding_player: chooser,
                                                presentation: crate::state::PendingResolutionPresentation {
                                                    source_object_id: top.id,
                                                    candidates: candidates
                                                        .iter()
                                                        .map(|(object_id, _)| *object_id)
                                                        .collect(),
                                                    min: 1,
                                                    max: 1,
                                                    ordered: false,
                                                    prompt: prompt.clone(),
                                                    choice_kind: rv1::ChoiceKind::PermanentObjects,
                                                    unique_names: false,
                                                },
                                                continuation: crate::state::ResolutionContinuation::EquipmentReceiptAttach {
                                                    attachment: Box::new(
                                                        crate::state::PendingEquipmentReceiptAttach {
                                                            stack,
                                                            equipment,
                                                            created_tokens: previous
                                                                .produced_objects
                                                                .clone(),
                                                            candidate_generations: candidates,
                                                            original_offered_chooser: chooser,
                                                            delegated: false,
                                                            turn_order_cursor: chooser_index,
                                                        },
                                                    ),
                                                },
                                            });
                                        events.push(
                                            equipment_receipt_choice_event(self).ok_or(
                                                EngineError::ProgramInvariant(
                                                    "Equipment receipt chooser offer could not be rendered",
                                                ),
                                            )?,
                                        );
                                        events.push(ev_log(prompt));
                                        #[cfg(test)]
                                        trip_program_failpoint(ProgramFailpoint::OfferPublished)?;
                                        return Ok(ResolutionProgress::Parked);
                                    }
                                }
                            }
                        }
                    }
                    let outcome = if is_kyoshi_equipment_receipt_attach(
                        compiled,
                        instruction,
                        &effect,
                    ) {
                        EffectOutcome::Continue
                    } else if target_context.has_bindings && target_context.targets.is_empty() {
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
                    capture_program_outputs_for_stack(
                        self,
                        top,
                        compiled,
                        &mut program_state,
                        &outputs,
                        &result,
                    )?;
                    program_state.in_flight = None;
                    program_state.in_flight_produced_object_provenance = None;
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
            CompiledChooseRequirement::SourceCanReceivePlusOnePlusOneCounters => {
                super::pump_counters::can_put_counters(
                    self,
                    top,
                    &[],
                    &tricerules_card_model::primitives::EffectSubject::Source,
                )
            }
            CompiledChooseRequirement::TriggerObjectCanBeLinkedExiled => {
                super::resolve_zone_effect_subject(
                    self,
                    top,
                    &[],
                    &tricerules_card_model::primitives::EffectSubject::TriggerObject,
                )
                .is_some()
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
        let branch = self.program_choose_branch_at_occurrence(stack, occurrence, branch_index)?;
        let original_chooser_live = self
            .state
            .player_idx(original_chooser)
            .is_some_and(|idx| !self.state.players[idx].has_lost);
        if (!original_chooser_live && branch.cost != ResolutionCost::None)
            || !self.program_choose_requirement_holds(&stack.item, &branch.requirement)
            || !self.program_choose_draw_is_selectable(&stack.item, &stack.previous_result, &branch)
            || !self.program_choose_cost_is_selectable(&stack.item, original_chooser, &branch.cost)
        {
            return Err(EngineError::Illegal(
                "that program Choose branch is no longer legal",
            ));
        }
        Ok(branch)
    }

    /// Resolve a saved branch occurrence without rechecking its current selectable state. The
    /// departure reconciler uses this to recover the exact cost before deciding whether changed
    /// candidate generations require a fresh offer or the Otherwise arm.
    pub(in crate::engine) fn program_choose_branch_at_occurrence(
        &self,
        stack: &ParkedStackResolution,
        occurrence: &crate::state::ProgramChooseOccurrence,
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
                super::resolution_choice_delegate_options(
                    &self.state,
                    stack.item.controller,
                    choose.original_chooser,
                    candidates,
                )
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
                            && self.program_choose_cost_is_selectable(
                                &stack.item,
                                choose.original_chooser,
                                &branch.cost,
                            )
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
                            ResolutionCost::PutHandCardOnLibraryBottom => (
                                rv1::ResolutionBranchCostKind::PutHandCardOnLibraryBottom,
                                "Put a card on the bottom of your library".to_string(),
                            ),
                            ResolutionCost::Blight { count } => (
                                rv1::ResolutionBranchCostKind::Blight,
                                format!("Blight {count}"),
                            ),
                            ResolutionCost::TapPermanents { count, .. } => (
                                rv1::ResolutionBranchCostKind::TapPermanents,
                                format!("tap {count} matching permanents"),
                            ),
                            ResolutionCost::ExileGraveyardCard { .. } => (
                                rv1::ResolutionBranchCostKind::ExileGraveyardCard,
                                "exile a matching card from your graveyard".to_string(),
                            ),
                            ResolutionCost::DiscardCard {
                                filter: Some(
                                    tricerules_card_model::primitives::CardTypeFilter::Creature,
                                ),
                            } => (
                                rv1::ResolutionBranchCostKind::DiscardCard,
                                "discard a creature card".to_string(),
                            ),
                            ResolutionCost::SacrificePermanent { .. } => (
                                rv1::ResolutionBranchCostKind::SacrificePermanent,
                                "sacrifice a matching permanent".to_string(),
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
            crate::state::PendingProgramChooseStage::PayingHandCard { .. }
            | crate::state::PendingProgramChooseStage::PayingGraveyardCardCost { .. }
            | crate::state::PendingProgramChooseStage::PayingDiscardCardCost { .. }
            | crate::state::PendingProgramChooseStage::PayingBlight { .. }
            | crate::state::PendingProgramChooseStage::PayingPermanentCost { .. }
            | crate::state::PendingProgramChooseStage::ChoosingCommanderDestination { .. } => {
                return None;
            }
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

/// This runtime continuation belongs only to Kyoshi Battle Fan's exact compiled receipt
/// consumer. The model compiler separately rejects widened owners and consumer shapes.
fn is_kyoshi_equipment_receipt_attach(
    compiled: &CompiledProgram,
    instruction: &tricerules_card_model::primitives::CompiledInstruction,
    effect: &SpellEffectKind,
) -> bool {
    use tricerules_card_model::{AbilityDefinitionId, AbilityId, CardFaceId, ProgramOwnerId};

    let expected_owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "kyoshi_battle_fan".into(),
        face_id: CardFaceId::new("kyoshi_battle_fan").expect("static face id"),
        ability_path: vec![AbilityId::new("triggered_01").expect("static ability id")],
    });
    compiled.owner == expected_owner
        && matches!(
            effect,
            SpellEffectKind::AttachEquipment {
                equipment: tricerules_card_model::primitives::EffectSubject::Source,
                creature: tricerules_card_model::primitives::EffectSubject::PreviousEffectObjects,
            }
        )
        && instruction.input.is_some_and(|slot_id| {
            compiled.slot(slot_id).is_some_and(|slot| {
                matches!(slot.result_type, ResultType::CreatedTokenBatch { .. })
            })
        })
}

pub(in crate::engine) fn equipment_receipt_choice_event(
    engine: &GameEngine,
) -> Option<rv1::RuledEvent> {
    let pending = engine.state.pending_resolution.as_ref()?;
    if !matches!(
        pending.continuation,
        crate::state::ResolutionContinuation::EquipmentReceiptAttach { .. }
    ) {
        return None;
    }
    let candidate_names = pending
        .presentation
        .candidates
        .iter()
        .map(|object_id| {
            engine
                .state
                .objects
                .get(object_id)
                .and_then(|object| engine.registry.get(&object.card_id))
                .map(|definition| definition.name.to_string())
                .unwrap_or_else(|| "Ally token".to_string())
        })
        .collect();
    Some(rv1::RuledEvent {
        ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
            rv1::ResolutionChoiceRequired {
                resolution_prompt_id: pending.resolution_prompt_id.get(),
                deciding_player_id: pending.deciding_player,
                source_object_id: pending.presentation.source_object_id,
                prompt_text: pending.presentation.prompt.clone(),
                choice_kind: rv1::ChoiceKind::PermanentObjects as i32,
                candidate_object_ids: pending.presentation.candidates.clone(),
                candidate_names,
                min: pending.presentation.min,
                max: pending.presentation.max,
                ..Default::default()
            },
        )),
    })
}

/// Return the target records owned by this exact compiled Program projection. Both instruction
/// role projection and targeted-player chooser resolution must use the same owner-local slice.
fn program_owner_target_records<'a>(
    top: &'a StackItem,
    compiled: &CompiledProgram,
) -> Result<&'a [StackTarget], EngineError> {
    match &compiled.target_projection {
        tricerules_card_model::primitives::ProgramTargetProjection::OwnerLocal => {
            Ok(top.targets.as_slice())
        }
        tricerules_card_model::primitives::ProgramTargetProjection::ModeLocal { mode_id } => top
            .chosen_modes
            .iter()
            .find(|chosen| chosen.mode_id == *mode_id)
            .map(|chosen| chosen.targets.as_slice())
            .ok_or(EngineError::ProgramInvariant(
                "mode-local target program has no matching chosen mode",
            )),
    }
}

/// Project the exact target records owned by one compiled instruction address. Mode-local
/// programs read only their stable mode's target records and never fall back to the stack-wide
/// flattened target list.
fn program_target_context(
    top: &StackItem,
    compiled: &CompiledProgram,
    instruction_address: u32,
) -> Result<ProgramTargetContext, EngineError> {
    let mut bindings = compiled
        .target_bindings
        .iter()
        .filter(|binding| binding.instruction_address == instruction_address)
        .collect::<Vec<_>>();
    let input_group_indices = compiled
        .group_inputs
        .iter()
        .filter(|input| input.instruction_address == instruction_address)
        .map(|input| input.group_index)
        .collect::<Vec<_>>();
    if bindings.is_empty() && input_group_indices.is_empty() {
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

    let chosen_targets = program_owner_target_records(top, compiled)?;
    let role_group_indices = bindings
        .iter()
        .map(|binding| binding.target_group_index)
        .collect::<Vec<_>>();
    let mut projected_group_indices = role_group_indices.clone();
    for group_index in input_group_indices {
        if !projected_group_indices.contains(&group_index) {
            projected_group_indices.push(group_index);
        }
    }
    let selected_targets = chosen_targets
        .iter()
        .filter(|target| projected_group_indices.contains(&target.group_index))
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
        // Group inputs supply non-target operands to an instruction. They extend the exact
        // generation-bound target context used by recipients without turning that instruction
        // into a targeted effect or changing its role-indexed target bindings.
        has_bindings: !bindings.is_empty(),
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
                ));
            }
        };
    }
    Ok(block)
}

#[cfg(test)]
fn capture_program_outputs(
    compiled: &CompiledProgram,
    run: &mut ProgramRunState,
    outputs: &[ResultSlotId],
    result: &EffectResult,
) -> Result<(), EngineError> {
    capture_program_outputs_inner(None, compiled, run, outputs, result)
}

fn capture_program_outputs_for_stack(
    engine: &crate::engine::GameEngine,
    top: &StackItem,
    compiled: &CompiledProgram,
    run: &mut ProgramRunState,
    outputs: &[ResultSlotId],
    result: &EffectResult,
) -> Result<(), EngineError> {
    capture_program_outputs_inner(Some((engine, top)), compiled, run, outputs, result)
}

fn validate_physical_output_cardinality(
    result: &EffectResult,
    cardinality: &Cardinality,
    invariant: &'static str,
) -> Result<(), EngineError> {
    if result.produced_objects.len() < cardinality.min as usize
        || cardinality
            .max
            .is_some_and(|max| result.produced_objects.len() > max as usize)
    {
        return Err(EngineError::ProgramInvariant(invariant));
    }
    Ok(())
}

fn optional_physical_output(
    result: &EffectResult,
    cardinality: &Cardinality,
    invariant: &'static str,
) -> Result<Option<crate::state::TriggerObjectRef>, EngineError> {
    validate_physical_output_cardinality(result, cardinality, invariant)?;
    Ok(result.produced_objects.first().copied())
}

fn capture_program_outputs_inner(
    stack_context: Option<(&crate::engine::GameEngine, &StackItem)>,
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
            ResultType::CounteredAbilitySource { cardinality } => {
                ProgramValue::CounteredAbilitySource(optional_physical_output(
                    result,
                    cardinality,
                    "CounterTargetAbility result exceeded its compiled cardinality",
                )?)
            }
            ResultType::PermanentSelection { .. } => {
                ProgramValue::PermanentSelection(result.produced_objects.clone())
            }
            ResultType::ManifestedPermanent { cardinality } => {
                ProgramValue::ManifestedPermanent(optional_physical_output(
                    result,
                    cardinality,
                    "Manifest Dread result exceeded its compiled cardinality",
                )?)
            }
            ResultType::HandSelection { .. } => {
                ProgramValue::HandSelection(result.selected_hand_cards.clone())
            }
            ResultType::CardResultCohort { .. }
            | ResultType::TargetCardResultCohort { .. }
            | ResultType::LinkedExileCardResultCohort { .. } => {
                ProgramValue::CardResultCohort(crate::state::CardResultCohort {
                    cards: result.cards.clone(),
                    program_discard_result: None,
                })
            }
            ResultType::CastPaymentCohort { .. } => {
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
                    ));
                }
            },
            ResultType::RevealedLibraryCohort => match result.program_value.as_ref() {
                Some(value @ ProgramValue::RevealedLibraryCohort(_)) => value.clone(),
                _ => {
                    return Err(EngineError::ProgramInvariant(
                        "cohort reveal did not publish its typed result",
                    ));
                }
            },
            ResultType::CardPartition => match result.program_value.as_ref() {
                Some(value @ ProgramValue::CardPartition(_)) => value.clone(),
                _ => {
                    return Err(EngineError::ProgramInvariant(
                        "cohort split did not publish its typed partition",
                    ));
                }
            },
            ResultType::TokenCopyBatch => {
                if result.produced_object_provenance
                    != Some(crate::state::ProducedObjectProvenance::TokenCreationBatch)
                {
                    return Err(EngineError::ProgramInvariant(
                        "token-copy instruction did not publish its exact token batch",
                    ));
                }
                ProgramValue::TokenCopyBatch(result.produced_objects.clone())
            }
            ResultType::ExiledObject { cardinality } => {
                ProgramValue::ExiledObject(optional_physical_output(
                    result,
                    cardinality,
                    "Koya Exile result exceeded its compiled cardinality",
                )?)
            }
            ResultType::CreatedTokenBatch { cardinality } => {
                validate_physical_output_cardinality(
                    result,
                    cardinality,
                    "CreateTokens result exceeded its compiled cardinality",
                )?;
                ProgramValue::CreatedTokenBatch(result.produced_objects.clone())
            }
            ResultType::TriggerObjectLinkedExileReceipt {
                producer_ability_id,
                linked_exile_id,
                cardinality,
            } => {
                let (engine, top) = stack_context.ok_or(EngineError::ProgramInvariant(
                    "linked-Exile receipt capture lacks its exact stack occurrence",
                ))?;
                let receipt = optional_physical_output(
                    result,
                    cardinality,
                    "linked-Exile receipt exceeded its compiled cardinality",
                )?;
                let key = super::zones::linked_exile_key(
                    engine,
                    top,
                    linked_exile_id.clone(),
                    producer_ability_id.clone(),
                )
                .map_err(|_| {
                    EngineError::ProgramInvariant(
                        "linked-Exile receipt could not resolve its exact ability occurrence",
                    )
                })?;
                let receipt = receipt.filter(|receipt| {
                    engine
                        .state
                        .linked_exile_records
                        .get(&key)
                        .is_some_and(|records| {
                            records.iter().any(|record| {
                                record.object_id == receipt.object_id
                                    && record.zone_change_generation
                                        == receipt.zone_change_generation
                            })
                        })
                        && engine
                            .state
                            .objects
                            .get(&receipt.object_id)
                            .is_some_and(|object| object.zone == Zone::Exile)
                        && engine
                            .state
                            .zone_change_generation
                            .get(&receipt.object_id)
                            .copied()
                            .unwrap_or(0)
                            == receipt.zone_change_generation
                });
                ProgramValue::TriggerObjectLinkedExileReceipt(receipt)
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

fn validate_program_unit_inputs(
    engine: &GameEngine,
    top: &StackItem,
    compiled: &CompiledProgram,
) -> Result<(), EngineError> {
    validate_program_cast_payment_import(top, compiled)?;
    if compiled
        .required_inputs
        .contains(&ProgramRequiredInput::TriggerTargetingStackObject)
        && top.trigger_context.targeting_stack_object.is_none()
    {
        return Err(EngineError::ProgramInvariant(
            "resolution program is missing its captured targeting stack object",
        ));
    }
    if compiled
        .required_inputs
        .contains(&ProgramRequiredInput::TriggerAffectedPlayer)
        && top.trigger_context.affected_player.is_none()
    {
        return Err(EngineError::ProgramInvariant(
            "resolution program is missing its captured affected player",
        ));
    }
    if compiled
        .required_inputs
        .contains(&ProgramRequiredInput::TriggerObservedObject)
    {
        if top.trigger_context.observed_object.is_none() {
            return Err(EngineError::ProgramInvariant(
                "resolution program is missing its captured observed-object reference",
            ));
        }
        if engine.trigger_object_token_copy_snapshot(top).is_none() {
            return Err(EngineError::ProgramInvariant(
                "resolution program cannot resolve its captured observed-object copy snapshot",
            ));
        }
    }
    if compiled
        .required_inputs
        .contains(&ProgramRequiredInput::CapturedTriggerObjectRef)
        && top.trigger_context.observed_object.is_none()
    {
        return Err(EngineError::ProgramInvariant(
            "resolution program is missing its captured delayed-object reference",
        ));
    }
    if compiled
        .required_inputs
        .contains(&ProgramRequiredInput::TriggerLinkedExileObject)
        && top.trigger_context.observed_object.is_none()
    {
        return Err(EngineError::ProgramInvariant(
            "resolution program is missing its linked-Exile trigger-object reference",
        ));
    }
    Ok(())
}

fn validate_program_cast_payment_import(
    top: &StackItem,
    compiled: &CompiledProgram,
) -> Result<(), EngineError> {
    use tricerules_card_model::primitives::{CardResultAction, Cardinality};

    let mut imports = compiled
        .slots
        .iter()
        .filter(|slot| matches!(&slot.result_type, ResultType::CastPaymentCohort { .. }));
    let Some(slot) = imports.next() else {
        return Ok(());
    };
    let expected_type = ResultType::CastPaymentCohort {
        action: CardResultAction::Discard,
        cardinality: Cardinality {
            min: 1,
            max: Some(1),
        },
    };
    if slot.result_type != expected_type || imports.any(|slot| slot.result_type != expected_type) {
        return Err(EngineError::ProgramInvariant(
            "compiled cast-payment import has an unsupported result descriptor",
        ));
    }
    let [entry] = top.payment_result.cards.as_slice() else {
        return Err(EngineError::ProgramInvariant(
            "cast-payment import lacks its exact committed discard snapshot",
        ));
    };
    if entry.action != CardResultAction::Discard
        || !matches!(
            entry.provenance,
            crate::state::CardResultProvenance::ActionFacts
        )
    {
        return Err(EngineError::ProgramInvariant(
            "cast-payment import does not contain canonical discard action facts",
        ));
    }
    Ok(())
}

fn is_program_source_sacrifice_reflexive(effect: &SpellEffectKind) -> bool {
    matches!(
        effect,
        SpellEffectKind::CreateReflexiveTrigger {
            when: Some(
                tricerules_card_model::primitives::ResolutionReceiptCondition::Sacrificed {
                    object: tricerules_card_model::primitives::ConditionObjectRef::Source,
                }
            ),
            ..
        }
    )
}

/// Project an explicit typed source-sacrifice input into the existing receipt consumed by the
/// reflexive-trigger gate. This path is deliberately limited to the one compiled result shape
/// and never infers success from ambient previous-result state.
fn program_source_sacrifice_reflexive_receipt(
    compiled: &CompiledProgram,
    run: &ProgramRunState,
    input: Option<ResultSlotId>,
    top: &StackItem,
) -> Result<Option<crate::state::TriggerObjectRef>, EngineError> {
    use tricerules_card_model::primitives::{CardResultAction, Cardinality, PlayerRecipient};

    let slot_id = input.ok_or(EngineError::ProgramInvariant(
        "source-sacrifice reflexive child is missing its typed input",
    ))?;
    let slot = compiled.slot(slot_id).ok_or(EngineError::ProgramInvariant(
        "source-sacrifice reflexive input slot disappeared",
    ))?;
    let expected_type = ResultType::CardResultCohort {
        action: CardResultAction::Sacrifice,
        who: PlayerRecipient::Controller,
        cardinality: Cardinality {
            min: 0,
            max: Some(1),
        },
    };
    if slot.result_type != expected_type {
        return Err(EngineError::ProgramInvariant(
            "source-sacrifice reflexive input has the wrong compiled result type",
        ));
    }
    let value = run
        .slots
        .get(slot_id.0 as usize)
        .and_then(Option::as_ref)
        .ok_or(EngineError::ProgramInvariant(
            "source-sacrifice reflexive input is unavailable",
        ))?;
    let ProgramValue::CardResultCohort(cohort) = value else {
        return Err(EngineError::ProgramInvariant(
            "source-sacrifice reflexive input is not a card-result cohort",
        ));
    };
    let source_id = top
        .source_permanent_id
        .ok_or(EngineError::ProgramInvariant(
            "source-sacrifice reflexive child has no exact source object",
        ))?;
    source_sacrifice_receipt_from_cohort(cohort, source_id, top.source_zone_change, top.controller)
}

fn source_sacrifice_receipt_from_cohort(
    cohort: &crate::state::CardResultCohort,
    source_id: crate::state::ObjectId,
    source_generation: u64,
    instructed_player: crate::state::PlayerId,
) -> Result<Option<crate::state::TriggerObjectRef>, EngineError> {
    if cohort.cards.is_empty() {
        return Ok(None);
    }
    let [entry] = cohort.cards.as_slice() else {
        return Err(EngineError::ProgramInvariant(
            "source-sacrifice reflexive input has invalid cardinality",
        ));
    };
    let committed_input_matches = matches!(
        entry.provenance,
        crate::state::CardResultProvenance::CommittedMovement {
            input_zone: Zone::Battlefield,
            input_generation,
            ..
        } if input_generation == source_generation
    );
    if entry.action != tricerules_card_model::primitives::CardResultAction::Sacrifice
        || entry.affected_player != instructed_player
        || entry.object_id != source_id
        || !committed_input_matches
    {
        return Err(EngineError::ProgramInvariant(
            "source-sacrifice reflexive input does not prove the instructed source movement",
        ));
    }
    Ok(Some(crate::state::TriggerObjectRef {
        object_id: source_id,
        zone_change_generation: source_generation,
        controller_at_event: instructed_player,
    }))
}

fn program_result_condition_holds(
    engine: &GameEngine,
    top: &StackItem,
    run: &ProgramRunState,
    condition: &CompiledProgramResultCondition,
) -> Result<bool, EngineError> {
    match condition {
        CompiledProgramResultCondition::CardResultCount {
            result,
            card_type,
            min,
            max,
        } => {
            let Some(ProgramValue::CardResultCohort(result)) =
                run.slots.get(result.0 as usize).and_then(Option::as_ref)
            else {
                return Err(EngineError::ProgramInvariant(
                    "card result count condition read an unavailable or wrong-typed slot",
                ));
            };
            let count = result
                .cards
                .iter()
                .filter(|entry| match card_type {
                    Some(card_type) => entry.matched_card_types.contains(card_type),
                    None => true,
                })
                .count()
                .min(u32::MAX as usize) as u32;
            Ok(min.is_none_or(|minimum| count >= minimum)
                && max.is_none_or(|maximum| count <= maximum))
        }
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
        CompiledProgramResultCondition::CounteredAbilitySourceMatches { result } => {
            let Some(ProgramValue::CounteredAbilitySource(Some(source))) =
                run.slots.get(result.0 as usize).and_then(Option::as_ref)
            else {
                return match run.slots.get(result.0 as usize).and_then(Option::as_ref) {
                    Some(ProgramValue::CounteredAbilitySource(None)) => Ok(false),
                    _ => Err(EngineError::ProgramInvariant(
                        "countered ability source condition read an unavailable or wrong-typed slot",
                    )),
                };
            };
            if engine
                .state
                .zone_change_generation
                .get(&source.object_id)
                .copied()
                .unwrap_or(0)
                != source.zone_change_generation
                || !engine
                    .state
                    .objects
                    .get(&source.object_id)
                    .is_some_and(|object| object.zone == Zone::Battlefield)
            {
                return Ok(false);
            }
            let filter = TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![
                    PermanentTypeFilter::Artifact,
                    PermanentTypeFilter::Creature,
                    PermanentTypeFilter::Planeswalker,
                ],
                ..TargetFilter::default()
            };
            Ok(super::super::targeting::object_matches_mass_filter(
                engine,
                source.object_id,
                &filter,
            ))
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
                        &engine.state,
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
            let selected = &cx.previous_effect_result.selected_hand_cards;
            super::hand_library_return::begin_program_hand_library_return(cx, selected)
        }
        _ => Err(EngineError::Illegal(
            "operation is not a resolution-program-only leaf",
        )),
    }
}

#[cfg(test)]
mod countered_ability_source_tests {
    use super::*;
    use crate::state::{EffectResult, ProgramRunState, TriggerObjectRef};
    use tricerules_card_model::primitives::{
        Cardinality, CompiledBlock, CompiledSlot, EffectContext, ProgramTargetProjection,
    };

    fn ability_owner() -> tricerules_card_model::ProgramOwnerId {
        tricerules_card_model::ProgramOwnerId::ability(tricerules_card_model::AbilityDefinitionId {
            card_id: "stack_ability_program_fixture".into(),
            face_id: tricerules_card_model::CardFaceId::new("stack_ability_program_fixture")
                .unwrap(),
            ability_path: vec![tricerules_card_model::AbilityId::new("triggered_01").unwrap()],
        })
    }

    fn compiled_counter_program() -> CompiledProgram {
        CompiledProgram {
            owner: ability_owner(),
            context: EffectContext::Ability,
            target_projection: ProgramTargetProjection::OwnerLocal,
            required_inputs: Vec::new(),
            target_bindings: Vec::new(),
            group_inputs: Vec::new(),
            body: CompiledBlock {
                steps: Vec::new(),
                yields: Vec::new(),
            },
            slots: vec![CompiledSlot {
                id: ResultSlotId(0),
                name: "receipt".into(),
                result_type: ResultType::CounteredAbilitySource {
                    cardinality: Cardinality {
                        min: 0,
                        max: Some(1),
                    },
                },
            }],
        }
    }

    fn run_with_slot(slot: Option<ProgramValue>) -> ProgramRunState {
        ProgramRunState {
            owner: ability_owner(),
            pending_owners: Vec::new(),
            frames: Vec::new(),
            slots: vec![slot],
            in_flight: None,
            in_flight_produced_object_provenance: None,
        }
    }

    fn fixture_engine_and_top() -> (GameEngine, StackItem) {
        let mut engine = GameEngine::new_with_default_decks(
            tricerules_cards::registry::global(),
            91_277,
            &[0, 1],
            20,
        )
        .expect("game engine");
        engine.state.opening = None;
        let card = *engine.state.players[0]
            .hand
            .first()
            .expect("default deck dealt a card");
        engine
            .state
            .objects
            .get_mut(&card)
            .expect("fixture card object")
            .card_id = "brainstorm".into();
        super::super::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            card,
            Zone::Stack,
            None,
        )
        .expect("fixture stack object");
        let mut top = engine.observer_return_item(card, 0);
        top.id = card;
        top.controller = 0;
        top.card_id = "brainstorm".into();
        (engine, top)
    }

    fn put_card_on_battlefield(
        engine: &mut GameEngine,
        object_id: ObjectId,
        card_id: &str,
    ) -> TriggerObjectRef {
        engine
            .state
            .objects
            .get_mut(&object_id)
            .expect("fixture permanent object")
            .card_id = card_id.into();
        super::super::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            object_id,
            Zone::Battlefield,
            Some(0),
        )
        .expect("fixture permanent enters battlefield");
        TriggerObjectRef {
            object_id,
            zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
            controller_at_event: 0,
        }
    }

    #[test]
    fn countered_ability_source_capture_preserves_optional_exact_receipt_and_checks_cardinality() {
        let compiled = compiled_counter_program();
        let mut run = run_with_slot(None);
        let source = TriggerObjectRef {
            object_id: 91_278,
            zone_change_generation: 7,
            controller_at_event: 0,
        };
        let mut result = EffectResult::default();
        result.produced_objects.push(source);
        capture_program_outputs(&compiled, &mut run, &[ResultSlotId(0)], &result)
            .expect("the handler's exact source reference is captured");
        assert!(matches!(
            run.slots[0],
            Some(ProgramValue::CounteredAbilitySource(Some(actual))) if actual == source
        ));

        run.slots[0] = None;
        result.produced_objects.clear();
        capture_program_outputs(&compiled, &mut run, &[ResultSlotId(0)], &result)
            .expect("a completed counter with no permanent source writes a completed None result");
        assert!(matches!(
            run.slots[0],
            Some(ProgramValue::CounteredAbilitySource(None))
        ));

        result.produced_objects = vec![source, source];
        assert!(matches!(
            capture_program_outputs(&compiled, &mut run, &[ResultSlotId(0)], &result),
            Err(EngineError::ProgramInvariant(
                "CounterTargetAbility result exceeded its compiled cardinality"
            ))
        ));
    }

    #[test]
    fn countered_ability_source_condition_uses_current_generation_and_live_permanent_types() {
        use tricerules_card_model::primitives::CompiledProgramResultCondition;

        let (mut engine, top) = fixture_engine_and_top();
        let objects = engine.state.players[0]
            .library
            .iter()
            .take(4)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(objects.len(), 4);
        let artifact = put_card_on_battlefield(&mut engine, objects[0], "sol_ring");
        let creature = put_card_on_battlefield(&mut engine, objects[1], "grizzly_bears");
        let planeswalker =
            put_card_on_battlefield(&mut engine, objects[2], "teferi,_temporal_pilgrim");
        let enchantment = put_card_on_battlefield(&mut engine, objects[3], "curiosity");
        let holds = |engine: &GameEngine, receipt: Option<TriggerObjectRef>| {
            program_result_condition_holds(
                engine,
                &top,
                &run_with_slot(Some(ProgramValue::CounteredAbilitySource(receipt))),
                &CompiledProgramResultCondition::CounteredAbilitySourceMatches {
                    result: ResultSlotId(0),
                },
            )
        };

        for receipt in [artifact, creature, planeswalker] {
            assert!(
                holds(&engine, Some(receipt)).unwrap(),
                "current source {:?}, zone {:?}, generation {:?}, characteristics {:?}, card id {:?}, registry {:?}",
                receipt,
                engine.state.objects.get(&receipt.object_id).map(|object| object.zone),
                engine.state.zone_change_generation.get(&receipt.object_id),
                engine.characteristics(receipt.object_id),
                engine
                    .state
                    .objects
                    .get(&receipt.object_id)
                    .map(|object| object.card_id.as_str()),
                engine
                    .state
                    .objects
                    .get(&receipt.object_id)
                    .and_then(|object| engine.registry.get(&object.card_id))
            );
        }
        assert!(!holds(&engine, Some(enchantment)).unwrap());
        assert!(!holds(&engine, None).unwrap());
        assert!(matches!(
            program_result_condition_holds(
                &engine,
                &top,
                &run_with_slot(None),
                &CompiledProgramResultCondition::CounteredAbilitySourceMatches {
                    result: ResultSlotId(0),
                }
            ),
            Err(EngineError::ProgramInvariant(
                "countered ability source condition read an unavailable or wrong-typed slot"
            ))
        ));

        let mut stale = artifact;
        stale.zone_change_generation = stale.zone_change_generation.saturating_sub(1);
        assert!(!holds(&engine, Some(stale)).unwrap());
        super::super::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            creature.object_id,
            Zone::Graveyard,
            None,
        )
        .expect("creature leaves battlefield");
        assert!(!holds(&engine, Some(creature)).unwrap());
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

    fn late_nested_cast_payment_fixture(
        registry: &'static tricerules_card_model::CardRegistry,
        seed: u64,
    ) -> GameEngine {
        let decks = Some(vec![
            vec!["late_nested_import_fixture".to_string(); 30],
            vec!["forest".to_string(); 30],
        ]);
        let mut engine = GameEngine::new(registry, seed, &[0, 1], 20, decks, true)
            .expect("late nested cast-payment fixture engine");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;
        engine
            .apply_scenario_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
                        cast_method: rv1::CastMethod::Normal as i32,
                        source: Some(rv1::CastSource {
                            expected_zone_change_generation: None,
                            location: Some(rv1::cast_source::Location::HandIndex(0)),
                        }),
                        cost_selections: vec![rv1::CostSelection {
                            cost_index: 0,
                            selection: Some(rv1::cost_selection::Selection::HandIndex(1)),
                        }],
                        ..Default::default()
                    })),
                },
            )
            .expect("fixture spell pays its mandatory discard");
        engine
    }

    #[test]
    fn late_nested_cast_payment_import_fails_atomically_before_visible_prefix() {
        use tricerules_card_model::primitives::CardResultAction;

        let card = r#"(
            id: "late_nested_import_fixture",
            name: "Late Nested Import Fixture",
            face_id: "late_nested_import_fixture",
            mana_cost: "{0}",
            types: ["Instant"],
            additional_costs: [DiscardCard],
            spell_program: Some((body: (steps: [
                (id: "visible_prefix", bind: [], input: None,
                    kind: Perform(Draw(who: Controller, count: 1))),
                (id: "nested_import", bind: [], input: None,
                    kind: If(condition: Void,
                        yes: (steps: [], yields: []),
                        no: (steps: [
                            (id: "import_paid", bind: ["paid"], input: None,
                                kind: ImportCastPayment),
                        ], yields: []),
                    )),
            ], yields: []), target_bindings: [])),
        )"#;
        let registry = Box::leak(Box::new(
            tricerules_card_model::CardRegistry::from_chunks_and_tokens(
                &[
                    card,
                    include_str!("../../../../tricerules-cards/data/forest.ron"),
                ],
                &[],
            )
            .expect("late nested cast-payment fixture compiles"),
        ));
        let compiled = registry
            .compiled_program("late_nested_import_fixture", "late_nested_import_fixture")
            .expect("fixture Program compiled");
        assert!(compiled.slots.iter().any(|slot| matches!(
            &slot.result_type,
            ResultType::CastPaymentCohort {
                action: CardResultAction::Discard,
                ..
            }
        )));

        for corruption in ["empty", "wrong action", "two entries"] {
            let mut engine = late_nested_cast_payment_fixture(registry, 91_260);
            let top = engine.state.stack.last_mut().expect("cast spell on stack");
            assert_eq!(top.payment_result.cards.len(), 1);
            match corruption {
                "empty" => top.payment_result.cards.clear(),
                "wrong action" => {
                    top.payment_result.cards[0].action = CardResultAction::Sacrifice;
                }
                "two entries" => {
                    top.payment_result
                        .cards
                        .push(top.payment_result.cards[0].clone());
                }
                _ => unreachable!(),
            }

            let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
            engine
                .apply_scenario_command(
                    0,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                    },
                )
                .expect("first player passes toward resolution");
            let before_resolution = command_snapshot(&engine);
            let failure = engine.apply_scenario_command(
                1,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            );
            assert!(
                matches!(failure, Err(EngineError::ProgramInvariant(_))),
                "the {corruption} snapshot must fail during resolver preflight, got {failure:?}"
            );
            assert!(
                command_snapshot(&engine) == before_resolution,
                "the {corruption} failure restores the command snapshot"
            );
            assert_eq!(
                engine.state.turn_history.current.player(0).cards_drawn,
                draws_before,
                "the earlier visible Draw instruction did not execute"
            );
            assert!(engine.state.pending_resolution.is_none());
        }
    }

    #[test]
    fn mode_local_group_input_projects_only_its_mode_and_group_without_target_role() {
        use crate::state::{ChosenMode, StackTarget};
        use tricerules_card_model::primitives::ProgramTargetProjection;

        let engine = program_resolution_fixture();
        let mut compiled = engine
            .registry
            .compiled_program("heated_argument", "heated_argument")
            .expect("the existing Heated Argument Program has a group input")
            .clone();
        let [group_input] = compiled.group_inputs.as_slice() else {
            panic!("Heated Argument has one target-controller group input");
        };
        assert_eq!(group_input.group_index, 0);
        let rider_address = group_input.instruction_address;
        let mode_id = tricerules_card_model::ModeId::new("heated_mode").unwrap();
        compiled.target_projection = ProgramTargetProjection::ModeLocal {
            mode_id: mode_id.clone(),
        };
        compiled.owner = tricerules_card_model::ProgramOwnerId::face_mode(
            "heated_argument",
            tricerules_card_model::CardFaceId::new("heated_argument").unwrap(),
            mode_id.clone(),
        );

        let flat_group_zero = engine.state.players[0].hand[0];
        let flat_group_one = engine.state.players[0].hand[1];
        let selected_mode_group_zero = engine.state.players[0].hand[2];
        let selected_mode_group_one = engine.state.players[0].hand[3];
        let other_mode_group_zero = engine.state.players[1].hand[0];
        let target = |object_id, group_index, damage_amount| StackTarget {
            object_id,
            group_index,
            damage_amount,
            kind: rv1::TargetRefKind::Permanent as i32,
            zone_change_generation: None,
            required_controller: None,
        };
        let mut top = engine.state.stack.last().expect("resolving spell").clone();
        top.targets = vec![
            target(flat_group_zero, 0, 10),
            target(flat_group_one, 1, 11),
        ];
        top.chosen_modes = vec![
            ChosenMode {
                mode_id: mode_id.clone(),
                targets: vec![
                    target(selected_mode_group_zero, 0, 20),
                    target(selected_mode_group_one, 1, 21),
                ],
            },
            ChosenMode {
                mode_id: tricerules_card_model::ModeId::new("other_mode").unwrap(),
                targets: vec![target(other_mode_group_zero, 0, 30)],
            },
        ];

        let projected = program_target_context(&top, &compiled, rider_address)
            .expect("the named mode supplies the target group input");
        assert_eq!(projected.targets, vec![selected_mode_group_zero]);
        assert_eq!(projected.target_damage, vec![20]);
        assert_eq!(projected.target_group_indices, vec![0]);
        assert!(projected.targets_by_role.is_empty());
        assert!(
            !projected.has_bindings,
            "the target-controller rider consumes a group input without becoming a target role"
        );
    }

    #[test]
    fn mode_local_targeted_choose_uses_only_its_mode_target_and_keeps_original_chooser() {
        use crate::state::{ChosenMode, StackTarget};
        use tricerules_card_model::primitives::ProgramTargetProjection;
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        const CARD: &str = r#"(id: "mode_local_targeted_choose_fixture",
 name: "Mode Local Targeted Choose Fixture", face_id: "mode_local_targeted_choose_fixture",
 mana_cost: "{0}", types: ["Instant"],
 modal_spell: (min_modes: 2, max_modes: 2, modes: [
  (mode_id: "focused", presentation: Fallback, effects: [],
   program: Some((body: (steps: [
    (id: "choose", bind: [], input: None, kind: Choose(choice: "choose",
     chooser: TargetedPlayer(group_index: 0, kind: OpponentPlayer), optional: true,
     branches: [(branch_id: "damage", presentation: Fallback, cost: None,
      requirement: Always, body: (steps: [
       (id: "damage", bind: [], input: None, kind: Perform(DamagePlayer(
        who: TargetedPlayer(group_index: 0, kind: OpponentPlayer), amount: 3)))
      ], yields: []))], otherwise: (steps: [], yields: [])))
   ], yields: []), target_bindings: [
    (instruction_id: "damage", role: Target, target_group_index: 0)
   ])),
   targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose the focused opponent", effect_indices: [])]))),
  (mode_id: "other", presentation: Fallback,
   effects: [],
   program: Some((body: (steps: [
    (id: "other_choose", bind: [], input: None, kind: Choose(choice: "other_choice",
     chooser: TargetedPlayer(group_index: 0, kind: OpponentPlayer), optional: true,
     branches: [(branch_id: "other_damage", presentation: Fallback, cost: None,
      requirement: Always, body: (steps: [
       (id: "other_damage", bind: [], input: None,
        kind: Perform(DamagePlayer(amount: 1,
         who: TargetedPlayer(group_index: 0, kind: OpponentPlayer))))
      ], yields: []))], otherwise: (steps: [], yields: [])))
   ], yields: []), target_bindings: [
    (instruction_id: "other_damage", role: Target, target_group_index: 0)
   ])),
   targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose the other opponent", effect_indices: [])])))
 ]))"#;

        fn fixture(
            seed: u64,
        ) -> (
            GameEngine,
            StackItem,
            Vec<tricerules_card_model::ProgramOwnerId>,
        ) {
            let card_id = "mode_local_targeted_choose_fixture";
            let registry = Box::leak(Box::new(
                tricerules_card_model::CardRegistry::from_chunks_and_tokens(&[CARD], &[])
                    .expect("ModeLocal target chooser fixture compiles"),
            ));
            let decks = Some(
                (0..4)
                    .map(|_| vec![card_id.to_string(); 30])
                    .collect::<Vec<_>>(),
            );
            let mut engine = GameEngine::new(registry, seed, &[0, 1, 2, 3], 20, decks, true)
                .expect("four-player ModeLocal fixture engine");
            engine.state.opening = None;
            engine.state.turn_step = TurnStep::Main1;
            engine.state.active_player_idx = 0;
            engine.state.priority_idx = 0;

            let spell = *engine.state.players[0]
                .hand
                .first()
                .expect("fixture spell is in hand");
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
            top.source_owner = Some(0);
            top.cast_by = Some(0);
            top.cast_occurrence = Some(crate::state::StackObjectRef {
                object_id: spell,
                zone_change_generation: Some(generation),
            });
            let target = |player: i32| StackTarget {
                object_id: player as u32,
                group_index: 0,
                damage_amount: 0,
                kind: rv1::TargetRefKind::Player as i32,
                zone_change_generation: None,
                required_controller: None,
            };
            top.targets = vec![target(3)];
            top.chosen_modes = vec![
                ChosenMode {
                    mode_id: tricerules_card_model::ModeId::new("focused").unwrap(),
                    targets: vec![target(1)],
                },
                ChosenMode {
                    mode_id: tricerules_card_model::ModeId::new("other").unwrap(),
                    targets: vec![target(2)],
                },
            ];
            let owners = engine
                .compiled_resolution_program_owners(&top)
                .expect("ModeLocal owner lookup")
                .expect("selected mode has its exact Program owner");
            assert_eq!(owners.len(), 2);
            assert!(matches!(
                &engine
                    .registry
                    .compiled_program_for_owner(&owners[0])
                    .expect("first selected mode Program")
                    .target_projection,
                ProgramTargetProjection::ModeLocal { mode_id }
                    if mode_id.as_str() == "focused"
            ));
            assert!(matches!(
                &engine
                    .registry
                    .compiled_program_for_owner(&owners[1])
                    .expect("second selected mode Program")
                    .target_projection,
                ProgramTargetProjection::ModeLocal { mode_id }
                    if mode_id.as_str() == "other"
            ));
            engine.state.stack.push(top.clone());
            (engine, top, owners)
        }

        let (mut engine, top, owners) = fixture(91_271);
        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        let initial_prompt_id = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();
        assert_ne!(
            initial_prompt_id, 0,
            "targeted chooser receives a real prompt ID"
        );
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            1,
            "the focused mode target controls the chooser despite conflicting group zeroes"
        );
        assert!(matches!(
            &engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .continuation,
            ResolutionContinuation::ProgramChoose { choose, .. }
                if choose.original_chooser == 1
        ));
        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        assert_eq!(
            engine.state.players[1].life, 17,
            "focused mode's chooser branch damages its own group zero"
        );
        assert_eq!(engine.state.players[0].life, 20);
        assert_eq!(engine.state.players[2].life, 20);
        assert_eq!(
            engine.state.players[3].life, 20,
            "the stack-global group zero is not used by the focused mode program"
        );
        let other_mode_pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(other_mode_pending.deciding_player, 2);
        assert!(matches!(
            &other_mode_pending.continuation,
            ResolutionContinuation::ProgramChoose { choose, .. }
                if choose.original_chooser == 2
        ));
        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        assert_eq!(engine.state.players[0].life, 20);
        assert_eq!(engine.state.players[1].life, 17);
        assert_eq!(engine.state.players[2].life, 19);
        assert_eq!(engine.state.players[3].life, 20);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());

        let (mut departed, top, owners) = fixture(91_272);
        nested_start(
            &mut departed,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        let original_prompt_id = departed
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();
        assert_ne!(
            original_prompt_id, 0,
            "the original chooser prompt has a real ID"
        );
        departed.state.players[1].has_lost = true;
        let mut events = Vec::new();
        departed
            .refresh_program_choose_departure(&mut events)
            .expect("the controller receives a delegate-selection prompt");
        departed
            .publish_resolution_prompt_ids(&mut events)
            .expect("delegate-selection prompt receives a real ID");
        let pending = departed.state.pending_resolution.as_ref().unwrap();
        assert_eq!(pending.deciding_player, 0);
        assert_ne!(pending.resolution_prompt_id.get(), 0);
        assert!(matches!(
            &pending.continuation,
            ResolutionContinuation::ProgramChoose {
                choose: crate::state::PendingProgramChoose {
                    original_chooser: 1,
                    stage: crate::state::PendingProgramChooseStage::ChoosingDelegate { candidates },
                    ..
                },
                ..
            } if candidates == &[Some(2), Some(3)]
        ));
        nested_answer(&mut departed, Decision::SelectBranch, 0, Vec::new());
        assert!(matches!(
            &departed
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .continuation,
            ResolutionContinuation::ProgramChoose {
                choose: crate::state::PendingProgramChoose {
                    original_chooser: 1,
                    delegate_selected: true,
                    ..
                },
                ..
            }
        ));
        assert_eq!(
            departed
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            2
        );
    }

    #[test]
    fn captured_delayed_object_input_requires_presence_but_not_current_object() {
        let engine = program_resolution_fixture();
        let mut top = engine.state.stack.last().expect("resolving spell").clone();
        let compiled = CompiledProgram {
            owner: tricerules_card_model::ProgramOwnerId::face(
                "captured_delayed_object_fixture",
                tricerules_card_model::CardFaceId::new("front").unwrap(),
            ),
            context: tricerules_card_model::primitives::EffectContext::Ability,
            target_projection:
                tricerules_card_model::primitives::ProgramTargetProjection::OwnerLocal,
            required_inputs: vec![ProgramRequiredInput::CapturedTriggerObjectRef],
            target_bindings: Vec::new(),
            group_inputs: Vec::new(),
            body: CompiledBlock {
                steps: Vec::new(),
                yields: Vec::new(),
            },
            slots: Vec::new(),
        };

        assert!(matches!(
            validate_program_unit_inputs(&engine, &top, &compiled),
            Err(EngineError::ProgramInvariant(
                "resolution program is missing its captured delayed-object reference"
            ))
        ));

        top.trigger_context.observed_object = Some(crate::state::TriggerObjectRef {
            object_id: u32::MAX,
            zone_change_generation: u64::MAX,
            controller_at_event: 1,
        });
        assert!(validate_program_unit_inputs(&engine, &top, &compiled).is_ok());
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
            in_flight_produced_object_provenance: None,
        };

        assert!(program_result_condition_holds(
            &engine,
            &top,
            &run,
            &CompiledProgramResultCondition::DiscardOutcome {
                result: result_slot,
                outcome: tricerules_card_model::DiscardResultOutcome::ChosenToPay,
            },
        )
        .expect("outcome condition evaluates"));
        assert!(!program_result_condition_holds(
            &engine,
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
    fn card_result_count_uses_typed_cohort_entry_count_and_rejects_unavailable_values() {
        use crate::state::{CardResultCohort, CardResultEntry, CardResultProvenance};
        use tricerules_card_model::primitives::CardResultAction;

        let engine = program_resolution_fixture();
        let top = engine.state.stack.last().expect("resolving spell").clone();
        let entry = |object_id| CardResultEntry {
            action: CardResultAction::Sacrifice,
            affected_player: 0,
            object_id,
            zone_change_generation: 0,
            matched_card_types: Vec::new(),
            provenance: CardResultProvenance::ActionFacts,
        };
        let cohort_value = |count| {
            ProgramValue::CardResultCohort(CardResultCohort {
                cards: [entry(1), entry(2)].into_iter().take(count).collect(),
                program_discard_result: None,
            })
        };
        let evaluate = |slot, min, max| {
            let run = ProgramRunState {
                owner: tricerules_card_model::ProgramOwnerId::face(
                    "brainstorm",
                    tricerules_card_model::CardFaceId::new("brainstorm").unwrap(),
                ),
                pending_owners: Vec::new(),
                frames: Vec::new(),
                slots: vec![slot],
                in_flight: None,
                in_flight_produced_object_provenance: None,
            };
            program_result_condition_holds(
                &engine,
                &top,
                &run,
                &CompiledProgramResultCondition::CardResultCount {
                    result: ResultSlotId(0),
                    card_type: None,
                    min,
                    max,
                },
            )
        };

        assert!(!evaluate(Some(cohort_value(0)), Some(1), None).unwrap());
        assert!(evaluate(Some(cohort_value(1)), Some(1), None).unwrap());
        assert!(evaluate(Some(cohort_value(2)), Some(2), Some(2)).unwrap());
        assert!(!evaluate(Some(cohort_value(1)), Some(2), None).unwrap());
        assert!(matches!(
            evaluate(
                Some(ProgramValue::CounterPaymentOutcome(
                    tricerules_card_model::primitives::CounterPaymentOutcome::Paid
                )),
                Some(1),
                None,
            ),
            Err(EngineError::ProgramInvariant(
                "card result count condition read an unavailable or wrong-typed slot"
            ))
        ));
        assert!(matches!(
            evaluate(None, Some(1), None),
            Err(EngineError::ProgramInvariant(
                "card result count condition read an unavailable or wrong-typed slot"
            ))
        ));
    }

    #[test]
    fn card_result_count_filters_captured_card_type_snapshot_without_live_lookup() {
        use crate::state::{CardResultCohort, CardResultEntry, CardResultProvenance};
        use tricerules_card_model::primitives::{CardResultAction, CardTypeFilter};

        let engine = program_resolution_fixture();
        let top = engine.state.stack.last().expect("resolving spell").clone();
        let cohort = ProgramValue::CardResultCohort(CardResultCohort {
            cards: vec![
                CardResultEntry {
                    action: CardResultAction::Discard,
                    affected_player: 17,
                    object_id: 9001,
                    zone_change_generation: 4,
                    matched_card_types: vec![CardTypeFilter::Nonland],
                    provenance: CardResultProvenance::ActionFacts,
                },
                CardResultEntry {
                    action: CardResultAction::Discard,
                    affected_player: 18,
                    object_id: 9002,
                    zone_change_generation: 5,
                    matched_card_types: vec![CardTypeFilter::Land],
                    provenance: CardResultProvenance::ActionFacts,
                },
            ],
            program_discard_result: None,
        });
        let run = ProgramRunState {
            owner: tricerules_card_model::ProgramOwnerId::face(
                "brainstorm",
                tricerules_card_model::CardFaceId::new("brainstorm").unwrap(),
            ),
            pending_owners: Vec::new(),
            frames: Vec::new(),
            slots: vec![Some(cohort)],
            in_flight: None,
            in_flight_produced_object_provenance: None,
        };
        let evaluate = |card_type| {
            program_result_condition_holds(
                &engine,
                &top,
                &run,
                &CompiledProgramResultCondition::CardResultCount {
                    result: ResultSlotId(0),
                    card_type,
                    min: Some(1),
                    max: Some(1),
                },
            )
        };

        assert!(evaluate(Some(CardTypeFilter::Nonland)).unwrap());
        assert!(evaluate(Some(CardTypeFilter::Land)).unwrap());
        assert!(!evaluate(Some(CardTypeFilter::Creature)).unwrap());
        assert!(
            !engine.state.objects.contains_key(&9001) && !engine.state.objects.contains_key(&9002),
            "the count uses captured type facts after the paid cards leave their original objects"
        );
    }

    #[test]
    fn cast_payment_import_preflight_rejects_malformed_snapshot_before_instruction_zero() {
        use crate::state::{CardResultEntry, CardResultProvenance};
        use tricerules_card_model::primitives::{
            CardResultAction, Cardinality, CompiledBlock, CompiledSlot, ProgramTargetProjection,
        };

        let engine = program_resolution_fixture();
        let mut top = engine.state.stack.last().expect("resolving spell").clone();
        let compiled = CompiledProgram {
            owner: tricerules_card_model::ProgramOwnerId::face(
                "brainstorm",
                tricerules_card_model::CardFaceId::new("brainstorm").unwrap(),
            ),
            context: tricerules_card_model::primitives::EffectContext::Spell,
            target_projection: ProgramTargetProjection::OwnerLocal,
            required_inputs: Vec::new(),
            target_bindings: Vec::new(),
            group_inputs: Vec::new(),
            body: CompiledBlock {
                steps: Vec::new(),
                yields: Vec::new(),
            },
            slots: vec![CompiledSlot {
                id: ResultSlotId(0),
                name: "paid".into(),
                result_type: ResultType::CastPaymentCohort {
                    action: CardResultAction::Discard,
                    cardinality: Cardinality {
                        min: 1,
                        max: Some(1),
                    },
                },
            }],
        };
        let entry = |action, provenance| CardResultEntry {
            action,
            affected_player: 23,
            object_id: 9001,
            zone_change_generation: 4,
            matched_card_types: vec![tricerules_card_model::primitives::CardTypeFilter::Nonland],
            provenance,
        };

        assert!(matches!(
            validate_program_unit_inputs(&engine, &top, &compiled),
            Err(EngineError::ProgramInvariant(
                "cast-payment import lacks its exact committed discard snapshot"
            ))
        ));
        top.payment_result.cards = vec![entry(
            CardResultAction::Sacrifice,
            CardResultProvenance::ActionFacts,
        )];
        assert!(matches!(
            validate_program_unit_inputs(&engine, &top, &compiled),
            Err(EngineError::ProgramInvariant(
                "cast-payment import does not contain canonical discard action facts"
            ))
        ));
        top.payment_result.cards = vec![entry(
            CardResultAction::Discard,
            CardResultProvenance::CommittedMovement {
                input_zone: Zone::Hand,
                input_generation: 3,
                actual_output_zone: Zone::Graveyard,
                output_visibility: crate::state::CardResultOutputVisibility::FaceUpPublic,
            },
        )];
        assert!(matches!(
            validate_program_unit_inputs(&engine, &top, &compiled),
            Err(EngineError::ProgramInvariant(
                "cast-payment import does not contain canonical discard action facts"
            ))
        ));
        top.payment_result.cards = vec![entry(
            CardResultAction::Discard,
            CardResultProvenance::ActionFacts,
        )];
        assert!(validate_program_unit_inputs(&engine, &top, &compiled).is_ok());
    }

    fn token_copy_batch_capture_fixture() -> (CompiledProgram, ProgramRunState) {
        use tricerules_card_model::primitives::{
            CompiledSlot, EffectContext, ProgramTargetProjection,
        };

        let owner = tricerules_card_model::ProgramOwnerId::face(
            "token_copy_batch_capture_fixture",
            tricerules_card_model::CardFaceId::new("front").unwrap(),
        );
        let compiled = CompiledProgram {
            owner: owner.clone(),
            context: EffectContext::Spell,
            target_projection: ProgramTargetProjection::OwnerLocal,
            required_inputs: Vec::new(),
            target_bindings: Vec::new(),
            group_inputs: Vec::new(),
            body: CompiledBlock {
                steps: Vec::new(),
                yields: Vec::new(),
            },
            slots: vec![CompiledSlot {
                id: ResultSlotId(0),
                name: "copies".into(),
                result_type: ResultType::TokenCopyBatch,
            }],
        };
        let run = ProgramRunState {
            owner,
            pending_owners: Vec::new(),
            frames: Vec::new(),
            slots: vec![None],
            in_flight: None,
            in_flight_produced_object_provenance: None,
        };
        (compiled, run)
    }

    fn created_token_batch_capture_fixture() -> (CompiledProgram, ProgramRunState) {
        use tricerules_card_model::primitives::{
            Cardinality, CompiledSlot, EffectContext, ProgramTargetProjection,
        };

        let owner = tricerules_card_model::ProgramOwnerId::ability(
            tricerules_card_model::AbilityDefinitionId {
                card_id: "kyoshi_battle_fan".into(),
                face_id: tricerules_card_model::CardFaceId::new("kyoshi_battle_fan").unwrap(),
                ability_path: vec![tricerules_card_model::AbilityId::new("triggered_01").unwrap()],
            },
        );
        let compiled = CompiledProgram {
            owner: owner.clone(),
            context: EffectContext::Ability,
            target_projection: ProgramTargetProjection::OwnerLocal,
            required_inputs: Vec::new(),
            target_bindings: Vec::new(),
            group_inputs: Vec::new(),
            body: CompiledBlock {
                steps: Vec::new(),
                yields: Vec::new(),
            },
            slots: vec![CompiledSlot {
                id: ResultSlotId(0),
                name: "created_tokens".into(),
                result_type: ResultType::CreatedTokenBatch {
                    cardinality: Cardinality { min: 0, max: None },
                },
            }],
        };
        let run = ProgramRunState {
            owner,
            pending_owners: Vec::new(),
            frames: Vec::new(),
            slots: vec![None],
            in_flight: None,
            in_flight_produced_object_provenance: None,
        };
        (compiled, run)
    }

    #[test]
    fn token_copy_batch_capture_rejects_missing_provenance_for_empty_result() {
        let (compiled, mut run) = token_copy_batch_capture_fixture();
        let result = crate::state::EffectResult::default();

        assert!(matches!(
            capture_program_outputs(&compiled, &mut run, &[ResultSlotId(0)], &result,),
            Err(EngineError::ProgramInvariant(
                "token-copy instruction did not publish its exact token batch"
            ))
        ));
        assert!(run.slots[0].is_none());
    }

    #[test]
    fn token_copy_batch_capture_keeps_a_proven_empty_batch_distinct_from_no_result() {
        let (compiled, mut run) = token_copy_batch_capture_fixture();
        let result = crate::state::EffectResult {
            produced_object_provenance: Some(
                crate::state::ProducedObjectProvenance::TokenCreationBatch,
            ),
            ..Default::default()
        };

        capture_program_outputs(&compiled, &mut run, &[ResultSlotId(0)], &result)
            .expect("an empty cohort with the executor's explicit batch marker is typed output");
        assert!(matches!(
            run.slots[0],
            Some(ProgramValue::TokenCopyBatch(ref objects)) if objects.is_empty()
        ));
    }

    #[test]
    fn equipment_created_token_batch_records_empty_completion_distinct_from_unwritten_slot() {
        let (compiled, mut run) = created_token_batch_capture_fixture();
        assert!(
            run.slots[0].is_none(),
            "before the producer, the slot is unwritten"
        );

        capture_program_outputs(
            &compiled,
            &mut run,
            &[ResultSlotId(0)],
            &crate::state::EffectResult::default(),
        )
        .expect("the exact Equipment receipt permits a completed empty token cohort");
        assert!(matches!(
            run.slots[0],
            Some(ProgramValue::CreatedTokenBatch(ref objects)) if objects.is_empty()
        ));
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
            validate_program_unit_inputs(&engine, &top, compiled),
            Err(EngineError::ProgramInvariant(
                "resolution program is missing its captured affected player"
            ))
        ));

        top.trigger_context.affected_player = Some(1);
        validate_program_unit_inputs(&engine, &top, compiled)
            .expect("the captured draw-step actor satisfies the input guard");
    }

    #[test]
    fn ward_program_requires_both_captured_inputs_but_not_a_live_targeting_object() {
        let engine = program_resolution_fixture();
        let compiled = CompiledProgram {
            owner: tricerules_card_model::ProgramOwnerId::face(
                "ward_input_guard_fixture",
                tricerules_card_model::CardFaceId::new("front").unwrap(),
            ),
            context: tricerules_card_model::primitives::EffectContext::Ability,
            target_projection:
                tricerules_card_model::primitives::ProgramTargetProjection::OwnerLocal,
            required_inputs: vec![
                ProgramRequiredInput::TriggerAffectedPlayer,
                ProgramRequiredInput::TriggerTargetingStackObject,
            ],
            target_bindings: Vec::new(),
            group_inputs: Vec::new(),
            body: CompiledBlock {
                steps: Vec::new(),
                yields: Vec::new(),
            },
            slots: Vec::new(),
        };
        let mut top = engine
            .state
            .stack
            .last()
            .expect("fixture stack item")
            .clone();
        top.trigger_context.targeting_stack_object = Some(crate::state::StackObjectRef {
            object_id: u32::MAX,
            zone_change_generation: Some(u64::MAX),
        });

        assert!(matches!(
            validate_program_unit_inputs(&engine, &top, &compiled),
            Err(EngineError::ProgramInvariant(
                "resolution program is missing its captured affected player"
            ))
        ));

        top.trigger_context.affected_player = Some(1);
        top.trigger_context.targeting_stack_object = None;
        assert!(matches!(
            validate_program_unit_inputs(&engine, &top, &compiled),
            Err(EngineError::ProgramInvariant(
                "resolution program is missing its captured targeting stack object"
            ))
        ));

        top.trigger_context.targeting_stack_object = Some(crate::state::StackObjectRef {
            object_id: u32::MAX,
            zone_change_generation: Some(u64::MAX),
        });
        validate_program_unit_inputs(&engine, &top, &compiled)
            .expect("an exact captured reference remains valid after its stack object leaves");
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

    #[test]
    fn mandatory_choose_with_one_free_live_branch_enters_it_and_runs_the_tail_once() {
        let card = r#"(id: "program_single_free_fixture", name: "Program Single Free Fixture",
            face_id: "program_single_free_fixture", mana_cost: "{0}", types: ["Instant"],
            spell_program: Some((body: (steps: [
                (id: "choose", kind: Choose(
                    choice: "only_free", chooser: Controller, optional: false,
                    branches: [(branch_id: "gain_one", presentation: Fallback,
                        cost: None, requirement: Always,
                        body: (steps: [(id: "branch", kind: Perform(GainLife(amount: 1)))], yields: []))],
                    otherwise: (steps: [], yields: []),
                )),
                (id: "tail", kind: Perform(GainLife(amount: 2))),
            ], yields: []))))"#;
        let card_id = "program_single_free_fixture";
        let registry = Box::leak(Box::new(
            tricerules_card_model::CardRegistry::from_chunks_and_tokens(&[card], &[])
                .expect("single-free program fixture compiles"),
        ));
        let decks = Some(vec![
            vec![card_id.to_string(); 30],
            vec![card_id.to_string(); 30],
        ]);
        let mut engine = GameEngine::new(registry, 91_206, &[0, 1], 20, decks, true)
            .expect("two-player program fixture");
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
        top.card_id = card_id.into();
        top.ability_text = None;
        top.is_copy = false;
        top.cast_by = Some(0);
        top.cast_occurrence = Some(crate::state::StackObjectRef {
            object_id: spell,
            zone_change_generation: Some(generation),
        });
        engine.state.stack.push(top.clone());

        let owners = engine
            .compiled_resolution_program_owners(&top)
            .expect("program owner lookup")
            .expect("fixture spell has a program owner");
        let run = engine
            .begin_program_run(&top, owners)
            .expect("fixture program starts");
        let mut events = Vec::new();
        let progress = engine
            .run_program_with_previous(
                &top,
                run,
                crate::state::EffectResult::default(),
                &mut events,
            )
            .expect("single live free branch resolves without a prompt");

        assert_eq!(progress, ResolutionProgress::Completed);
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            engine.state.players[0].life, 23,
            "branch and tail each resolve once"
        );
        assert!(!events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
        )));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::Log(log))
                        if log.text.starts_with("P0 chooses: ")
                ))
                .count(),
            1,
            "the mandatory one-live-branch selection is logged once"
        );
    }

    #[test]
    fn mandatory_choose_auto_entry_joins_child_yield_for_parent_and_tail_once() {
        const CARD: &str = r#"(id: "mandatory_joined_yield_fixture",
            name: "Mandatory Joined Yield Fixture",
            face_id: "mandatory_joined_yield_fixture", mana_cost: "{0}", types: ["Instant"],
            spell_program: Some((body: (steps: [
                (id: "choose", bind: ["joined"], kind: Choose(
                    choice: "only_free", chooser: Controller, optional: false,
                    branches: [(branch_id: "only", presentation: Fallback, cost: None,
                        requirement: Always,
                        body: (steps: [(id: "pick", bind: ["picked"], kind:
                            Perform(ChoosePermanents(
                                chooser: Controller,
                                filter: (kind: AnyPermanent, permanent_types: [Land]),
                                min: 1, max: 1,
                            )))], yields: ["picked"]))],
                    otherwise: (steps: [(id: "otherwise_pick", bind: ["otherwise_picked"], kind:
                        Perform(ChoosePermanents(
                            chooser: Controller,
                            filter: (kind: AnyPermanent, permanent_types: [Land]),
                            min: 1, max: 1,
                        )))], yields: ["otherwise_picked"],
                    ),
                )),
                (id: "untap", input: Some("joined"), kind: Perform(UntapChosenPermanents)),
                (id: "tail", kind: Perform(GainLife(amount: 2))),
            ], yields: []), target_bindings: [])))"#;
        let (mut engine, top, owners, islands) =
            nested_choose_fixture(CARD, "mandatory_joined_yield_fixture", 91_208, 2, &[]);
        let before_life = engine.state.players[0].life;
        let run = engine
            .begin_program_run(&top, owners)
            .expect("the mandatory joined-yield program starts");
        let mut first_offer = Vec::new();
        assert!(matches!(
            engine
                .run_program_with_previous(
                    &top,
                    run,
                    crate::state::EffectResult::default(),
                    &mut first_offer,
                )
                .expect("the single free branch auto-enters and parks at its child selection"),
            ResolutionProgress::Parked
        ));
        engine
            .publish_resolution_prompt_ids(&mut first_offer)
            .expect("the child permanent choice gets its prompt ID");

        let offers = first_offer
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(offers.len(), 1, "only the child selection is offered");
        assert_eq!(
            offers[0].choice_kind,
            rv1::ChoiceKind::PermanentObjects as i32,
            "mandatory auto-entry must not publish a branch-choice prompt"
        );
        assert_eq!(offers[0].candidate_object_ids, islands);
        assert_eq!(
            first_offer
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::Log(log))
                        if log.text.starts_with("P0 chooses: ")
                ))
                .count(),
            1,
            "the mandatory branch is logged once before the nested choice"
        );
        let (pending, _, program) = parked_program(&engine);
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::PermanentObjects
        );
        assert!(program
            .frames
            .iter()
            .flat_map(|frame| &frame.path)
            .any(|path| matches!(path.arm, ProgramArm::ChooseBranch(_))));

        let completed = nested_select_objects(&mut engine, vec![islands[0]]);
        assert!(
            !completed.events.iter().any(|event| matches!(
                &event.ev,
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
            )),
            "the joined result reaches the parent without another prompt"
        );
        assert!(!engine.state.objects[&islands[0]].tapped);
        assert!(engine.state.objects[&islands[1]].tapped);
        assert_eq!(
            engine.state.players[0].life,
            before_life + 2,
            "the parent consumes the joined selection and the tail runs exactly once"
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
    }

    fn controller_sacrifice_choose_fixture(
        source_only: bool,
    ) -> (
        GameEngine,
        StackItem,
        Vec<tricerules_card_model::ProgramOwnerId>,
        ObjectId,
        ObjectId,
    ) {
        controller_sacrifice_choose_fixture_with_optionality(source_only, true)
    }

    fn controller_sacrifice_choose_fixture_with_optionality(
        source_only: bool,
        optional: bool,
    ) -> (
        GameEngine,
        StackItem,
        Vec<tricerules_card_model::ProgramOwnerId>,
        ObjectId,
        ObjectId,
    ) {
        use crate::state::{TriggerAbilityOrigin, TriggerUseKey};

        let card = format!(
            r#"(
            id: "controller_sacrifice_program_fixture",
            name: "Controller Sacrifice Program Fixture",
            face_id: "controller_sacrifice_program_fixture",
            types: ["Creature"], power: 2, toughness: 2,
            triggered_abilities: [(
                ability_id: "triggered_01", presentation: Fallback,
                trigger: WhenSelfEntersBattlefield, effect: [],
                program: Some((body: (steps: [
                    (id: "choose", kind: Choose(
                    choice: "sacrifice", chooser: Controller, optional: {optional},
                        branches: [(
                            branch_id: "sacrifice_source", presentation: Fallback,
                            cost: SacrificePermanent(
                                filter: (kind: AnyPermanent, controller: You),
                                source_only: {source_only},
                            ), requirement: Always,
                            body: (steps: [(id: "branch_gain", kind: Perform(GainLife(amount: 2)))], yields: []),
                        )],
                        otherwise: (steps: [(id: "otherwise_gain", kind: Perform(GainLife(amount: 7)))], yields: []),
                    )),
                    (id: "tail_gain", kind: Perform(GainLife(amount: 3))),
                ], yields: []), target_bindings: [])),
            )],
        )"#
        );
        let card_id = "controller_sacrifice_program_fixture";
        let registry = Box::leak(Box::new(
            tricerules_card_model::CardRegistry::from_chunks_and_tokens(&[card.as_str()], &[])
                .expect("Controller sacrifice Program fixture compiles"),
        ));
        let decks = Some(vec![
            vec![card_id.to_string(); 30],
            vec![card_id.to_string(); 30],
            vec![card_id.to_string(); 30],
        ]);
        let mut engine = GameEngine::new(registry, 91_209, &[0, 1, 2], 20, decks, true)
            .expect("three-player Controller sacrifice fixture");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;

        let source = *engine.state.players[1]
            .hand
            .iter()
            .find(|object_id| engine.state.objects[object_id].card_id == card_id)
            .expect("fixture source is owned by player one");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            Some(0),
        )
        .expect("fixture source enters under player zero's control");
        let borrowed = *engine.state.players[1]
            .hand
            .iter()
            .find(|object_id| engine.state.objects[object_id].card_id == card_id)
            .expect("a second player-one copy is available as a controlled permanent");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            borrowed,
            Zone::Battlefield,
            Some(0),
        )
        .expect("the borrowed permanent enters under player zero's control");

        let face = registry
            .get(card_id)
            .and_then(|definition| definition.face(0))
            .expect("fixture face");
        let ability = face
            .triggered_abilities
            .first()
            .expect("fixture triggered ability")
            .clone();
        let definition = tricerules_card_model::AbilityDefinitionId {
            card_id: card_id.into(),
            face_id: face.face_id.clone(),
            ability_path: vec![ability.ability_id.clone()],
        };
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut top = engine.observer_return_item(source, 0);
        top.id = engine.state.next_object_id.saturating_add(1);
        top.controller = 0;
        top.card_id = card_id.into();
        top.ability_text = Some("Controller Sacrifice Program Fixture trigger".into());
        top.source_permanent_id = Some(source);
        top.source_owner = Some(engine.state.objects[&source].owner);
        top.source_zone_change = generation;
        top.ability_index = Some(0);
        top.triggered_ability = Some(ability);
        top.trigger_use_key = Some(TriggerUseKey {
            object_id: source,
            zone_change_generation: generation,
            ability_origin: TriggerAbilityOrigin::Printed(definition),
        });
        top.is_triggered = true;
        top.is_copy = false;
        let owners = engine
            .compiled_resolution_program_owners(&top)
            .expect("trigger program owner lookup")
            .expect("fixture ability has a compiled Program");
        engine.state.stack.push(top.clone());
        (engine, top, owners, source, borrowed)
    }

    fn controller_filtered_discard_choose_fixture() -> (
        GameEngine,
        StackItem,
        Vec<tricerules_card_model::ProgramOwnerId>,
        ObjectId,
        ObjectId,
    ) {
        use crate::state::{TriggerAbilityOrigin, TriggerUseKey};

        const CARD_ID: &str = "controller_filtered_discard_program_fixture";
        let card = r#"(
            id: "controller_filtered_discard_program_fixture",
            name: "Controller Filtered Discard Program Fixture",
            face_id: "controller_filtered_discard_program_fixture",
            types: ["Creature"], power: 2, toughness: 2,
            triggered_abilities: [(
                ability_id: "triggered_01", presentation: Fallback,
                trigger: WhenSelfEntersBattlefield, effect: [],
                program: Some((body: (steps: [
                    (id: "choose", kind: Choose(
                        choice: "discard", chooser: Controller, optional: true,
                        branches: [(
                            branch_id: "discard_creature", presentation: Fallback,
                            cost: DiscardCard(filter: Some(Creature)), requirement: Always,
                            body: (steps: [(id: "branch_gain", kind: Perform(GainLife(amount: 2)))],
                                yields: []),
                        )],
                        otherwise: (steps: [(id: "otherwise_gain", kind: Perform(GainLife(amount: 7)))],
                            yields: []),
                    )),
                    (id: "tail_gain", kind: Perform(GainLife(amount: 3))),
                ], yields: []), target_bindings: [])),
            )],
        )"#;
        let registry = Box::leak(Box::new(
            tricerules_card_model::CardRegistry::from_chunks_and_tokens(&[card], &[])
                .expect("the filtered-discard Program fixture compiles"),
        ));
        let decks = Some(vec![vec![CARD_ID.to_string(); 30]; 2]);
        let mut engine = GameEngine::new(registry, 91_247, &[0, 1], 20, decks, true)
            .expect("two-player filtered-discard Program fixture");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;

        let source = *engine.state.players[0]
            .hand
            .first()
            .expect("opening hand contains the fixture source");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            Some(0),
        )
        .expect("the fixture source enters the battlefield");
        let candidate = *engine.state.players[0]
            .hand
            .first()
            .expect("another fixture creature remains in hand");
        let other_hand_cards = engine.state.players[0].hand.clone();
        for object_id in other_hand_cards.into_iter().filter(|id| *id != candidate) {
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                object_id,
                Zone::Graveyard,
                None,
            )
            .expect("only the exact payment candidate remains in hand");
        }

        let face = registry
            .get(CARD_ID)
            .and_then(|definition| definition.face(0))
            .expect("fixture face");
        let ability = face
            .triggered_abilities
            .first()
            .expect("fixture triggered ability")
            .clone();
        let definition = tricerules_card_model::AbilityDefinitionId {
            card_id: CARD_ID.into(),
            face_id: face.face_id.clone(),
            ability_path: vec![ability.ability_id.clone()],
        };
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut top = engine.observer_return_item(source, 0);
        top.id = engine.state.next_object_id.saturating_add(1);
        top.controller = 0;
        top.card_id = CARD_ID.into();
        top.ability_text = Some("Controller Filtered Discard Program Fixture trigger".into());
        top.source_permanent_id = Some(source);
        top.source_owner = Some(engine.state.objects[&source].owner);
        top.source_zone_change = generation;
        top.ability_index = Some(0);
        top.triggered_ability = Some(ability);
        top.trigger_use_key = Some(TriggerUseKey {
            object_id: source,
            zone_change_generation: generation,
            ability_origin: TriggerAbilityOrigin::Printed(definition),
        });
        top.is_triggered = true;
        top.is_copy = false;
        let owners = engine
            .compiled_resolution_program_owners(&top)
            .expect("fixture trigger Program owner lookup")
            .expect("the synthetic trigger owns a compiled Program");
        engine.state.stack.push(top.clone());
        (engine, top, owners, source, candidate)
    }

    fn controller_tap_choose_fixture_for_players(
        player_ids: &[PlayerId],
    ) -> (
        GameEngine,
        StackItem,
        Vec<tricerules_card_model::ProgramOwnerId>,
        ObjectId,
    ) {
        use crate::state::{TriggerAbilityOrigin, TriggerUseKey};

        let card_id = "controller_tap_program_fixture";
        let card = r#"(
            id: "controller_tap_program_fixture",
            name: "Controller Tap Program Fixture",
            face_id: "controller_tap_program_fixture",
            types: ["Creature"], power: 2, toughness: 2,
            triggered_abilities: [(
                ability_id: "triggered_01", presentation: Fallback,
                trigger: WhenSelfEntersBattlefield, effect: [],
                program: Some((body: (steps: [
                    (id: "choose_tap", bind: [], input: None, kind: Choose(
                        choice: "tap_permanent", chooser: Controller, optional: true,
                        branches: [(
                            branch_id: "tap_a_permanent", presentation: Fallback,
                            cost: TapPermanents(
                                count: 1,
                                filter: (kind: AnyPermanent, controller: You),
                                exclude_source: false,
                            ), requirement: Always,
                            body: (steps: [(id: "branch_gain", kind: Perform(GainLife(amount: 2)))],
                                yields: []),
                        )],
                        otherwise: (steps: [(id: "otherwise_gain", kind: Perform(GainLife(amount: 4)))],
                            yields: []),
                    )),
                    (id: "parent_tail", bind: [], input: None,
                        kind: Perform(GainLife(amount: 3))),
                ], yields: []), target_bindings: [])),
            )],
        )"#;
        let registry = Box::leak(Box::new(
            tricerules_card_model::CardRegistry::from_chunks_and_tokens(&[card], &[])
                .expect("Controller tap Program fixture compiles"),
        ));
        let decks = Some(
            (0..player_ids.len())
                .map(|_| vec![card_id.to_string(); 30])
                .collect(),
        );
        let mut engine = GameEngine::new(registry, 91_210, player_ids, 20, decks, true)
            .expect("two-player Controller tap fixture");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;

        let source = *engine.state.players[0]
            .hand
            .iter()
            .find(|object_id| engine.state.objects[object_id].card_id == card_id)
            .expect("fixture source is owned by player zero");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            Some(player_ids[0]),
        )
        .expect("fixture source enters under player zero's control");

        let face = registry
            .get(card_id)
            .and_then(|definition| definition.face(0))
            .expect("fixture face");
        let ability = face
            .triggered_abilities
            .first()
            .expect("fixture triggered ability")
            .clone();
        let definition = tricerules_card_model::AbilityDefinitionId {
            card_id: card_id.into(),
            face_id: face.face_id.clone(),
            ability_path: vec![ability.ability_id.clone()],
        };
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut top = engine.observer_return_item(source, player_ids[0]);
        top.id = engine.state.next_object_id.saturating_add(1);
        top.controller = player_ids[0];
        top.card_id = card_id.into();
        top.ability_text = Some("Controller Tap Program Fixture trigger".into());
        top.source_permanent_id = Some(source);
        top.source_owner = Some(engine.state.objects[&source].owner);
        top.source_zone_change = generation;
        top.ability_index = Some(0);
        top.triggered_ability = Some(ability);
        top.trigger_use_key = Some(TriggerUseKey {
            object_id: source,
            zone_change_generation: generation,
            ability_origin: TriggerAbilityOrigin::Printed(definition),
        });
        top.is_triggered = true;
        top.is_copy = false;
        let owners = engine
            .compiled_resolution_program_owners(&top)
            .expect("trigger program owner lookup")
            .expect("fixture ability has a compiled Program");
        engine.state.stack.push(top.clone());
        (engine, top, owners, source)
    }

    fn controller_tap_choose_fixture() -> (
        GameEngine,
        StackItem,
        Vec<tricerules_card_model::ProgramOwnerId>,
        ObjectId,
    ) {
        controller_tap_choose_fixture_for_players(&[0, 1])
    }

    fn resolve_controller_tap_choose_to_prompt(engine: &mut GameEngine) {
        let mut events = Vec::new();
        assert!(matches!(
            engine
                .resolve_top_of_stack(&mut events)
                .expect("trigger resolves to its Program Choose"),
            ResolutionProgress::Parked
        ));
        engine
            .publish_resolution_prompt_ids(&mut events)
            .expect("Program Choose prompt gets its stable ID");
    }

    const EXILE_GROUP_INPUT_PROGRAM_FIXTURE: &str = r#"(
        id: "program_exile_group_input_fixture", name: "Program Exile Group Input Fixture",
        face_id: "program_exile_group_input_fixture", types: ["Instant"],
        spell_program: Some((body: (steps: [
            (id: "initial_damage", bind: [], input: None, kind: Perform(DamageTarget(
                amount: 1, target: (kind: Creature),
            ))),
            (id: "choose_exile", bind: [], input: None, kind: Choose(
                choice: "exile_graveyard_card", chooser: Controller, optional: true,
                branches: [(
                    branch_id: "exile_and_damage", presentation: Fallback,
                    cost: ExileGraveyardCard(filter: None), requirement: Always,
                    body: (steps: [(id: "damage_controller", bind: [], input: None,
                        kind: Perform(DamagePlayer(amount: 2,
                            who: ControllerOfTargetGroup(group_index: 0),
                        )))], yields: []),
                )],
                otherwise: (steps: [(id: "otherwise_gain", kind: Perform(GainLife(amount: 4)))],
                    yields: []),
            )),
            (id: "parent_tail", bind: [], input: None,
                kind: Perform(GainLife(amount: 3))),
        ], yields: []), target_bindings: [
            (instruction_id: "initial_damage", role: Target, target_group_index: 0),
        ])),
        targeting: Some((groups: [(
            min: 1, max: 1, prompt: "Choose target creature", effect_indices: [],
        )]))
    )"#;

    const RETURN_GROUP_INPUT_PROGRAM_FIXTURE: &str = r#"(
        id: "program_return_group_input_fixture", name: "Program Return Group Input Fixture",
        face_id: "program_return_group_input_fixture", types: ["Instant"],
        spell_program: Some((body: (steps: [
            (id: "damage_target", bind: [], input: None, kind: Perform(DamageTarget(
                amount: 1, target: (kind: Creature),
            ))),
            (id: "return_target", bind: [], input: None, kind: Perform(ReturnToOwnersHand(
                subject: Chosen((kind: Creature)),
            ))),
            (id: "damage_group_controller", bind: [], input: None,
                kind: Perform(DamagePlayer(amount: 2,
                    who: ControllerOfTargetGroup(group_index: 1),
                ))),
        ], yields: []), target_bindings: [
            (instruction_id: "damage_target", role: Target, target_group_index: 1),
            (instruction_id: "return_target", role: Subject, target_group_index: 0),
        ])),
        targeting: Some((groups: [
            (min: 1, max: 1, prompt: "Choose the creature to return", effect_indices: []),
            (min: 1, max: 1, prompt: "Choose the creature to damage", effect_indices: []),
        ]))
    )"#;

    type ExileGroupInputFixture = (
        GameEngine,
        StackItem,
        Vec<tricerules_card_model::ProgramOwnerId>,
        ObjectId,
        Option<ObjectId>,
        Option<ObjectId>,
        Option<ObjectId>,
    );

    fn exile_group_input_fixture(
        card_id: &str,
        target_groups: &[u32],
        with_graveyard_candidates: bool,
    ) -> ExileGroupInputFixture {
        let registry = Box::leak(Box::new(
            tricerules_card_model::CardRegistry::from_chunks_and_tokens(
                &[
                    EXILE_GROUP_INPUT_PROGRAM_FIXTURE,
                    RETURN_GROUP_INPUT_PROGRAM_FIXTURE,
                    include_str!("../../../../tricerules-cards/data/island.ron"),
                    include_str!("../../../../tricerules-cards/data/grizzly_bears.ron"),
                ],
                &[],
            )
            .expect("graveyard and target-group Program fixtures compile"),
        ));
        let player_ids = [10, 20, 30];
        let decks = Some(vec![
            vec![card_id.to_string(); 30],
            vec!["grizzly_bears".to_string(); 30],
            vec!["grizzly_bears".to_string(); 30],
        ]);
        let mut engine = GameEngine::new(registry, 91_244, &player_ids, 20, decks, true)
            .expect("three-player Program fixture engine");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;

        let target = *engine.state.players[1]
            .hand
            .first()
            .expect("target owner drew a creature");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            target,
            Zone::Battlefield,
            Some(player_ids[2]),
        )
        .expect("target creature enters under the third player's control");
        let target_generation = engine.state.zone_change_generation[&target];

        let (own_graveyard, other_graveyard, own_library) = if with_graveyard_candidates {
            let own = *engine.state.players[0]
                .library
                .front()
                .expect("own library has a candidate");
            let off_zone = *engine.state.players[0]
                .library
                .get(1)
                .expect("own library has a separate off-zone card");
            let other = *engine.state.players[1]
                .library
                .front()
                .expect("other player's library has a candidate");
            for object_id in [own, off_zone, other] {
                engine.state.objects.get_mut(&object_id).unwrap().card_id = "island".into();
            }
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                own,
                Zone::Graveyard,
                None,
            )
            .expect("own Island enters its owner's graveyard");
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                other,
                Zone::Graveyard,
                None,
            )
            .expect("other player's Island enters their graveyard");
            (Some(own), Some(other), Some(off_zone))
        } else {
            (None, None, None)
        };

        let spell = *engine.state.players[0]
            .hand
            .iter()
            .find(|object_id| engine.state.objects[object_id].card_id == card_id)
            .expect("fixture spell is in its controller's hand");
        move_object_to_zone(&mut engine.state, engine.registry, spell, Zone::Stack, None)
            .expect("fixture spell moves to stack");
        let source_generation = engine.state.zone_change_generation[&spell];
        let mut top = engine.observer_return_item(spell, player_ids[0]);
        top.id = spell;
        top.controller = player_ids[0];
        top.card_id = card_id.into();
        top.ability_text = None;
        top.is_copy = false;
        top.face_index = 0;
        top.source_owner = Some(player_ids[0]);
        top.cast_by = Some(player_ids[0]);
        top.cast_occurrence = Some(crate::state::StackObjectRef {
            object_id: spell,
            zone_change_generation: Some(source_generation),
        });
        top.targets = target_groups
            .iter()
            .map(|group_index| StackTarget {
                object_id: target,
                group_index: *group_index,
                damage_amount: 0,
                kind: rv1::TargetRefKind::Permanent as i32,
                zone_change_generation: Some(target_generation),
                required_controller: None,
            })
            .collect();
        let owners = engine
            .compiled_resolution_program_owners(&top)
            .expect("fixture owner lookup")
            .expect("fixture has a compiled Program");
        engine.state.stack.push(top.clone());
        (
            engine,
            top,
            owners,
            target,
            own_graveyard,
            other_graveyard,
            own_library,
        )
    }

    #[test]
    fn program_choose_exile_graveyard_pay_decline_empty_and_run_otherwise_tail_once() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let (mut paid, top, owners, _target, own_graveyard, _other_graveyard, _own_library) =
            exile_group_input_fixture("program_exile_group_input_fixture", &[0], true);
        let own_graveyard = own_graveyard.unwrap();
        let offered_generation = paid.state.zone_change_generation[&own_graveyard];
        nested_start(
            &mut paid,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        let branch_offer = paid
            .program_choose_choice_event()
            .expect("optional graveyard-exile branch is offered");
        let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(branch_offer)) =
            branch_offer.ev.as_ref()
        else {
            panic!("Program Choose publishes its branch offer")
        };
        assert_eq!(branch_offer.resolution_branches.len(), 1);
        assert_eq!(
            branch_offer.resolution_branches[0].cost_kind,
            rv1::ResolutionBranchCostKind::ExileGraveyardCard as i32
        );
        nested_answer(&mut paid, Decision::SelectBranch, 0, Vec::new());
        let payment = paid
            .state
            .pending_resolution
            .as_ref()
            .expect("graveyard payment is pending");
        assert_eq!(payment.deciding_player, 10);
        assert_eq!(
            payment.presentation.choice_kind,
            rv1::ChoiceKind::GraveyardCards
        );
        assert_eq!(payment.presentation.candidates, vec![own_graveyard]);
        assert_eq!((payment.presentation.min, payment.presentation.max), (0, 1));
        let completed = nested_select_objects(&mut paid, vec![own_graveyard]);
        assert_eq!(paid.state.objects[&own_graveyard].zone, Zone::Exile);
        assert!(paid.state.zone_change_generation[&own_graveyard] > offered_generation);
        assert_eq!(
            paid.state.players[0].life, 23,
            "the parent tail resolves once"
        );
        assert_eq!(
            paid.state.players[1].life, 20,
            "the target owner is not the recipient"
        );
        assert_eq!(
            paid.state.players[2].life, 18,
            "the target controller takes the rider"
        );
        let life_deltas = completed
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(life_deltas, vec![-2, 3]);
        assert!(completed.events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                if moved.object_id == own_graveyard
                    && moved.destination == rv1::permanent_moved::Destination::Exile as i32
        )));
        assert!(paid.state.pending_resolution.is_none());
        assert!(paid.state.stack.is_empty());

        let (mut declined, top, owners, ..) =
            exile_group_input_fixture("program_exile_group_input_fixture", &[0], true);
        let before = declined.state.players[0].life;
        nested_start(
            &mut declined,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        let declined_batch = nested_answer(&mut declined, Decision::Decline, 0, Vec::new());
        assert_eq!(declined.state.players[0].life, before + 7);
        assert_eq!(declined.state.players[2].life, 20);
        assert_eq!(
            declined_batch
                .events
                .iter()
                .filter_map(|event| match &event.ev {
                    Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![4, 3],
            "Otherwise and the parent tail each resolve once"
        );

        let (mut empty, top, owners, ..) =
            exile_group_input_fixture("program_exile_group_input_fixture", &[0], false);
        let before = empty.state.players[0].life;
        let run = empty
            .begin_program_run(&top, owners)
            .expect("Program begins");
        let mut events = Vec::new();
        assert_eq!(
            empty
                .run_program_with_previous(
                    &top,
                    run,
                    crate::state::EffectResult::default(),
                    &mut events,
                )
                .expect("no own graveyard candidate enters Otherwise and tail"),
            ResolutionProgress::Completed
        );
        assert_eq!(empty.state.players[0].life, before + 7);
        assert_eq!(
            events
                .iter()
                .filter_map(|event| match &event.ev {
                    Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![4, 3]
        );
        assert!(empty.state.pending_resolution.is_none());
    }

    #[test]
    fn program_choose_exile_payment_rejects_wrong_actor_and_unoffered_objects_atomically() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let (mut engine, top, owners, _target, own_graveyard, other_graveyard, own_library) =
            exile_group_input_fixture("program_exile_group_input_fixture", &[0], true);
        let own_graveyard = own_graveyard.unwrap();
        let other_graveyard = other_graveyard.unwrap();
        let own_library = own_library.unwrap();
        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        let prompt_id = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("graveyard payment is pending")
            .resolution_prompt_id
            .get();
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates,
            vec![own_graveyard]
        );
        let answer = |objects: Vec<ObjectId>| RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: prompt_id,
                    chosen_object_ids: objects,
                    ..Default::default()
                },
            )),
        };
        let reject_atomically =
            |engine: &mut GameEngine, actor, objects: Vec<ObjectId>, reason: &str| {
                let before = command_snapshot(engine);
                assert!(
                    engine
                        .apply_scenario_command(actor, &answer(objects))
                        .is_err(),
                    "{reason} exile answer is rejected"
                );
                assert!(
                    command_snapshot(engine) == before,
                    "{reason} rejection preserves the complete pending state"
                );
            };
        reject_atomically(&mut engine, 20, vec![own_graveyard], "wrong actor");
        reject_atomically(&mut engine, 10, vec![other_graveyard], "other owner");
        reject_atomically(
            &mut engine,
            10,
            vec![own_library],
            "wrong zone and unoffered list entry",
        );
        reject_atomically(
            &mut engine,
            10,
            vec![own_graveyard, own_graveyard],
            "duplicate/count",
        );

        *engine
            .state
            .zone_change_generation
            .entry(own_graveyard)
            .or_default() += 1;
        reject_atomically(&mut engine, 10, vec![own_graveyard], "stale generation");
    }

    #[test]
    fn program_choose_exile_departed_controller_drops_parked_resolution() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let (mut engine, top, owners, _target, own_graveyard, ..) =
            exile_group_input_fixture("program_exile_group_input_fixture", &[0], true);
        let own_graveyard = own_graveyard.unwrap();
        let starting_life = engine.state.players[0].life;
        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());

        let stack_before_refresh = engine
            .state
            .stack
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        engine.state.players[0].has_lost = true;
        let mut events = Vec::new();
        engine
            .refresh_program_choose_departure(&mut events)
            .expect("a departed Controller payer loses the parked spell resolution");

        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.players[0].life, starting_life);
        assert_eq!(engine.state.objects[&own_graveyard].zone, Zone::Graveyard);
        assert_eq!(
            engine
                .state
                .stack
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            stack_before_refresh,
            "private departure refresh preserves the fixture's preexisting stack entries"
        );
        assert!(!events
            .iter()
            .any(|event| matches!(&event.ev, Some(rv1::ruled_event::Ev::LifeChanged(_)))));
    }

    #[test]
    fn program_group_input_uses_nonzero_target_group_lki_after_return_to_owner_hand() {
        let (mut engine, top, owners, target, ..) =
            exile_group_input_fixture("program_return_group_input_fixture", &[0, 1], false);
        let captured_generation = engine.state.zone_change_generation[&target];
        let run = engine
            .begin_program_run(&top, owners)
            .expect("Program begins");
        let mut events = Vec::new();
        assert_eq!(
            engine
                .run_program_with_previous(
                    &top,
                    run,
                    crate::state::EffectResult::default(),
                    &mut events,
                )
                .expect("return and target-group rider resolve in order"),
            ResolutionProgress::Completed
        );
        assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
        assert!(engine.state.zone_change_generation[&target] > captured_generation);
        assert_eq!(
            engine.state.players[0].life, 20,
            "the spell controller is untouched"
        );
        assert_eq!(
            engine.state.players[1].life, 20,
            "the target owner is untouched"
        );
        assert_eq!(
            engine.state.players[2].life, 18,
            "the prior target controller takes damage"
        );
    }

    #[test]
    fn program_choose_tap_cost_offer_uses_existing_kind_and_human_text() {
        let (mut engine, top, owners, _source) = controller_tap_choose_fixture();
        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );

        let offer = engine
            .program_choose_choice_event()
            .expect("the tap-cost Program Choose offer is rendered");
        let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(offer)) = offer.ev.as_ref() else {
            panic!("Controller receives the ordinary Program Choose offer");
        };
        let [branch] = offer.resolution_branches.as_slice() else {
            panic!("the single tap-cost branch is offered");
        };
        assert_eq!(
            branch.cost_kind,
            rv1::ResolutionBranchCostKind::TapPermanents as i32
        );
        let normalized_label = branch.label.to_ascii_lowercase();
        assert!(
            normalized_label.contains("tap") && normalized_label.contains("permanent"),
            "the label describes tapping a permanent: {}",
            branch.label
        );
        assert!(
            !branch.label.contains('_'),
            "the public label does not expose an internal branch slug: {}",
            branch.label
        );
        assert_eq!(branch.cost_text, "tap 1 matching permanents");
        assert!(branch.selectable);
    }

    #[test]
    fn program_choose_tap_payment_commits_once_and_runs_branch_then_parent_tail_once() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let player_ids = [7, 13, 29];
        let (mut engine, _top, _owners, payer) =
            controller_tap_choose_fixture_for_players(&player_ids);
        let starting_life = engine.state.players[0].life;
        let starting_tap_action = engine.state.next_tap_action_id;
        resolve_controller_tap_choose_to_prompt(&mut engine);
        let (pending, _, _) = parked_program(&engine);
        assert_eq!(pending.deciding_player, player_ids[0]);

        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        let payment_offer = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("tap payment is pending");
        assert_eq!(payment_offer.deciding_player, player_ids[0]);
        assert_eq!(payment_offer.presentation.candidates, vec![payer]);
        assert_eq!(
            (
                payment_offer.presentation.min,
                payment_offer.presentation.max
            ),
            (0, 1)
        );
        let payment_prompt_id = payment_offer.resolution_prompt_id.get();

        let completed = nested_select_objects(&mut engine, vec![payer]);
        assert!(engine.state.objects[&payer].tapped);
        assert_eq!(
            engine.state.next_tap_action_id,
            starting_tap_action + 1,
            "one accepted payment commits one tap action"
        );
        assert_eq!(engine.state.players[0].life, starting_life + 5);
        let life_deltas = completed
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            life_deltas,
            vec![2, 3],
            "branch and parent tail each run once"
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());

        let before_duplicate = command_snapshot(&engine);
        let duplicate_payment = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: payment_prompt_id,
                    chosen_object_ids: vec![payer],
                    ..Default::default()
                },
            )),
        };
        assert!(
            engine
                .apply_scenario_command(player_ids[0], &duplicate_payment)
                .is_err(),
            "a completed tap payment prompt cannot be submitted again"
        );
        assert!(
            command_snapshot(&engine) == before_duplicate,
            "duplicate completion leaves the full command snapshot unchanged"
        );
    }

    #[test]
    fn program_choose_tap_decline_empty_payment_and_no_candidates_run_otherwise_then_tail_once() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        for empty_payment in [false, true] {
            let (mut engine, _top, _owners, _payer) = controller_tap_choose_fixture();
            let starting_life = engine.state.players[0].life;
            resolve_controller_tap_choose_to_prompt(&mut engine);
            let completed = if empty_payment {
                nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
                nested_select_objects(&mut engine, Vec::new())
            } else {
                nested_answer(&mut engine, Decision::Decline, 0, Vec::new())
            };

            assert_eq!(engine.state.players[0].life, starting_life + 7);
            let life_deltas = completed
                .events
                .iter()
                .filter_map(|event| match &event.ev {
                    Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(life_deltas, vec![4, 3]);
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.state.stack.is_empty());
        }

        let (mut engine, _top, _owners, source) = controller_tap_choose_fixture();
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .expect("the source leaves before its trigger resolves, leaving no controlled candidate");
        let starting_life = engine.state.players[0].life;
        let mut events = Vec::new();
        assert_eq!(
            engine
                .resolve_top_of_stack(&mut events)
                .expect("an unavailable tap cost enters Otherwise"),
            ResolutionProgress::Completed
        );
        assert_eq!(engine.state.players[0].life, starting_life + 7);
        let life_deltas = events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(life_deltas, vec![4, 3]);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
    }

    #[test]
    fn program_choose_tap_payment_rejects_invalid_answers_atomically() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let player_ids = [7, 13, 29];
        let (mut engine, _top, _owners, payer) =
            controller_tap_choose_fixture_for_players(&player_ids);
        let second_payer = *engine.state.players[0]
            .hand
            .iter()
            .find(|object_id| **object_id != payer)
            .expect("a second controlled permanent is available");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            second_payer,
            Zone::Battlefield,
            Some(player_ids[0]),
        )
        .expect("the second permanent enters under the nonconsecutive first seat ID");
        resolve_controller_tap_choose_to_prompt(&mut engine);
        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        let prompt_id = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("tap payment is pending")
            .resolution_prompt_id
            .get();
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates,
            vec![payer, second_payer]
        );
        let offered_second_payer_generation = engine.state.zone_change_generation[&second_payer];

        let command = |prompt_id, chosen_object_ids| RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: prompt_id,
                    chosen_object_ids,
                    ..Default::default()
                },
            )),
        };
        fn assert_rejected_atomically(
            engine: &mut GameEngine,
            actor: PlayerId,
            answer: RuledCommand,
            reason: &str,
        ) {
            let before = command_snapshot(engine);
            assert!(
                engine.apply_scenario_command(actor, &answer).is_err(),
                "{reason} payment answer is rejected"
            );
            assert!(
                command_snapshot(engine) == before,
                "{reason} payment rejection preserves the pending offer and game state"
            );
        }

        assert_rejected_atomically(
            &mut engine,
            player_ids[1],
            command(prompt_id, vec![payer]),
            "wrong actor",
        );
        assert_rejected_atomically(
            &mut engine,
            player_ids[0],
            command(prompt_id + 1, vec![payer]),
            "stale prompt",
        );
        assert_rejected_atomically(
            &mut engine,
            player_ids[0],
            command(prompt_id, vec![payer, payer]),
            "duplicate object",
        );
        assert_rejected_atomically(
            &mut engine,
            player_ids[0],
            command(prompt_id, vec![payer, second_payer]),
            "over-count",
        );

        let old_generation = engine.state.zone_change_generation[&payer];
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            payer,
            Zone::Graveyard,
            None,
        )
        .expect("the offered payer leaves before its answer");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            payer,
            Zone::Battlefield,
            Some(player_ids[0]),
        )
        .expect("the same physical card returns as a different incarnation");
        assert_ne!(old_generation, engine.state.zone_change_generation[&payer]);
        assert_rejected_atomically(
            &mut engine,
            player_ids[0],
            command(prompt_id, vec![payer]),
            "stale object incarnation",
        );

        assert_eq!(
            engine.state.zone_change_generation[&second_payer], offered_second_payer_generation,
            "the second payer is still the offered incarnation before the tapped check"
        );
        engine.state.objects.get_mut(&second_payer).unwrap().tapped = true;
        assert_rejected_atomically(
            &mut engine,
            player_ids[0],
            command(prompt_id, vec![second_payer]),
            "newly tapped object",
        );
        engine.state.objects.get_mut(&second_payer).unwrap().tapped = false;
        assert_eq!(
            engine.state.zone_change_generation[&second_payer], offered_second_payer_generation,
            "restoring the tap state keeps the offered incarnation current"
        );
        engine
            .state
            .objects
            .get_mut(&second_payer)
            .unwrap()
            .controller = player_ids[1];
        assert_eq!(
            engine.state.zone_change_generation[&second_payer], offered_second_payer_generation,
            "control changes do not make the second payer's zone generation stale"
        );
        assert_rejected_atomically(
            &mut engine,
            player_ids[0],
            command(prompt_id, vec![second_payer]),
            "control-changed object",
        );
    }

    #[test]
    fn program_choose_tap_owner_departure_routes_otherwise_or_reoffers_survivor() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let player_ids = [7, 13, 29];
        let payment_offer = |include_survivor: bool| {
            let (mut engine, top, owners, source) =
                controller_tap_choose_fixture_for_players(&player_ids);
            // Keep the resolving Controller alive while making the borrowed permanent the only
            // payer unless a second, Controller-owned permanent is requested below.
            engine.state.objects.get_mut(&source).unwrap().tapped = true;
            let borrowed = *engine.state.players[1]
                .hand
                .first()
                .expect("seat 13 owns an available permanent");
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                borrowed,
                Zone::Battlefield,
                Some(player_ids[0]),
            )
            .expect("seat 13's permanent is controlled by the resolving Controller");

            let survivor = if include_survivor {
                let own = *engine.state.players[0]
                    .hand
                    .first()
                    .expect("seat 7 owns another available permanent");
                move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    own,
                    Zone::Battlefield,
                    Some(player_ids[0]),
                )
                .expect("seat 7's surviving permanent enters under its owner's control");
                Some(own)
            } else {
                None
            };

            let starting_life = engine.state.players[0].life;
            let starting_tap_action = engine.state.next_tap_action_id;
            nested_start(
                &mut engine,
                &top,
                owners,
                crate::state::EffectResult::default(),
            );
            nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
            let pending = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("the selected branch offers its tap payment");
            assert_eq!(pending.deciding_player, player_ids[0]);
            assert!(pending.presentation.candidates.contains(&borrowed));
            assert!(
                !pending.presentation.candidates.contains(&source),
                "the tapped resolving source is not a legal payer"
            );
            match survivor {
                Some(own) => assert!(pending.presentation.candidates.contains(&own)),
                None => assert_eq!(pending.presentation.candidates, vec![borrowed]),
            }
            assert_eq!(engine.state.objects[&borrowed].owner, player_ids[1]);
            assert_eq!(engine.state.objects[&borrowed].controller, player_ids[0]);
            let prompt_id = pending.resolution_prompt_id.get();
            (
                engine,
                source,
                borrowed,
                survivor,
                starting_life,
                starting_tap_action,
                prompt_id,
            )
        };

        let (
            mut no_candidate,
            source,
            borrowed,
            _survivor,
            starting_life,
            starting_tap_action,
            _old_prompt_id,
        ) = payment_offer(false);
        no_candidate.state.players[1].has_lost = true;
        move_object_to_zone(
            &mut no_candidate.state,
            no_candidate.registry,
            borrowed,
            Zone::Exile,
            None,
        )
        .expect("the departed owner's controlled permanent leaves the battlefield");
        let mut departure = Vec::new();
        no_candidate
            .refresh_program_choose_departure(&mut departure)
            .expect("a tap payment with no surviving candidate enters Otherwise");
        assert_eq!(no_candidate.state.players[0].life, starting_life + 7);
        let life_deltas = departure
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            life_deltas,
            vec![4, 3],
            "Otherwise and the parent tail run once"
        );
        assert_eq!(no_candidate.state.next_tap_action_id, starting_tap_action);
        assert_eq!(no_candidate.state.objects[&borrowed].zone, Zone::Exile);
        assert_eq!(no_candidate.state.objects[&source].zone, Zone::Battlefield);
        assert!(no_candidate.state.objects[&source].tapped);
        assert!(no_candidate.state.pending_resolution.is_none());
        assert!(no_candidate.state.stack.is_empty());
        assert!(!departure.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
        )));

        let (
            mut surviving_candidate,
            _source,
            departed_candidate,
            survivor,
            starting_life,
            starting_tap_action,
            old_prompt_id,
        ) = payment_offer(true);
        let survivor = survivor.expect("the second offer retains one Controller-owned candidate");
        let survivor_generation = surviving_candidate.state.zone_change_generation[&survivor];
        surviving_candidate.state.players[1].has_lost = true;
        move_object_to_zone(
            &mut surviving_candidate.state,
            surviving_candidate.registry,
            departed_candidate,
            Zone::Exile,
            None,
        )
        .expect("the departed owner's offered permanent leaves the battlefield");
        let mut refreshed = Vec::new();
        surviving_candidate
            .refresh_program_choose_departure(&mut refreshed)
            .expect("the surviving Controller's payment offer is narrowed");
        surviving_candidate
            .publish_resolution_prompt_ids(&mut refreshed)
            .expect("the narrowed offer receives a fresh prompt ID");
        let pending = surviving_candidate
            .state
            .pending_resolution
            .as_ref()
            .expect("the surviving candidate keeps the payment stage parked");
        assert_eq!(pending.deciding_player, player_ids[0]);
        assert_ne!(pending.resolution_prompt_id.get(), old_prompt_id);
        assert_eq!(pending.presentation.candidates, vec![survivor]);
        let crate::state::ResolutionContinuation::ProgramChoose { choose, .. } =
            &pending.continuation
        else {
            panic!("the refreshed tap payment remains a Program Choose");
        };
        let crate::state::PendingProgramChooseStage::PayingPermanentCost {
            candidate_generations,
            ..
        } = &choose.stage
        else {
            panic!("the refreshed tap offer retains its permanent-cost stage");
        };
        assert!(candidate_generations.contains(&(survivor, survivor_generation)));

        let stale_answer = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: old_prompt_id,
                    chosen_object_ids: vec![departed_candidate],
                    ..Default::default()
                },
            )),
        };
        let before_stale_answer = command_snapshot(&surviving_candidate);
        assert!(surviving_candidate
            .apply_scenario_command(player_ids[0], &stale_answer)
            .is_err());
        assert!(command_snapshot(&surviving_candidate) == before_stale_answer);
        assert_eq!(surviving_candidate.state.players[0].life, starting_life);
        assert!(surviving_candidate.state.pending_resolution.is_some());

        let paid = nested_select_objects(&mut surviving_candidate, vec![survivor]);
        assert_eq!(surviving_candidate.state.players[0].life, starting_life + 5);
        assert!(surviving_candidate.state.objects[&survivor].tapped);
        assert_eq!(
            surviving_candidate.state.next_tap_action_id,
            starting_tap_action + 1
        );
        let life_deltas = paid
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            life_deltas,
            vec![2, 3],
            "paid body and parent tail each run once"
        );
        assert!(surviving_candidate.state.pending_resolution.is_none());
        assert!(surviving_candidate.state.stack.is_empty());
    }

    #[test]
    fn program_choose_sacrifice_permanent_cost_is_offered_to_controller() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let (mut engine, top, owners, source, _borrowed) =
            controller_sacrifice_choose_fixture(true);
        let starting_life = engine.state.players[0].life;
        assert_eq!(engine.state.objects[&source].owner, 1);
        assert_eq!(engine.state.objects[&source].controller, 0);
        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );

        let (pending, _, _) = parked_program(&engine);
        assert_eq!(pending.deciding_player, 0);
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::ResolutionBranch
        );
        let offer = engine
            .program_choose_choice_event()
            .expect("the sacrifice branch offer is rendered");
        let Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(offer)) = offer.ev.as_ref() else {
            panic!("Controller receives the ordinary Program Choose offer");
        };
        let [branch] = offer.resolution_branches.as_slice() else {
            panic!("the single sacrifice branch is offered");
        };
        assert_eq!(branch.branch_index, 0);
        assert_eq!(
            branch.cost_kind,
            rv1::ResolutionBranchCostKind::SacrificePermanent as i32
        );
        assert_eq!(branch.label, "Sacrifice a permanent");
        assert_eq!(branch.cost_text, "sacrifice a matching permanent");
        assert!(branch.selectable);

        let payment_batch = nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        let cost_offers = payment_batch
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(cost_offers.len(), 1);
        assert_eq!(
            cost_offers[0].choice_kind,
            rv1::ChoiceKind::CostObjects as i32
        );
        assert_eq!((cost_offers[0].min, cost_offers[0].max), (0, 1));
        assert_eq!(cost_offers[0].candidate_object_ids, vec![source]);
        let payment = &engine
            .state
            .pending_resolution
            .as_ref()
            .expect("sacrifice object payment is pending")
            .presentation;
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            0
        );
        assert_eq!(payment.choice_kind, rv1::ChoiceKind::CostObjects);
        assert_eq!((payment.min, payment.max), (0, 1));
        assert_eq!(payment.candidates, vec![source]);
        let payment_prompt_id = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();

        let completed = nested_select_objects(&mut engine, vec![source]);
        assert_eq!(engine.state.players[0].life, starting_life + 5);
        let life_deltas = completed
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            life_deltas,
            vec![2, 3],
            "branch and parent tail each resolve once"
        );
        assert_eq!(
            completed
                .events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::Log(log)) if log.text.starts_with("P0 sacrifices ")
                ))
                .count(),
            1,
            "the exact sacrifice receipt is emitted once"
        );
        assert_eq!(
            engine.state.players[1]
                .graveyard
                .iter()
                .filter(|object_id| **object_id == source)
                .count(),
            1,
            "the source is sacrificed once"
        );
        assert!(!engine.state.players[0].graveyard.contains(&source));
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());

        let replay = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: payment_prompt_id,
                    chosen_object_ids: vec![source],
                    ..Default::default()
                },
            )),
        };
        assert!(
            engine.apply_scenario_command(0, &replay).is_err(),
            "the completed payment occurrence cannot be replayed"
        );
        assert_eq!(engine.state.players[0].life, starting_life + 5);
        assert_eq!(
            engine.state.players[1]
                .graveyard
                .iter()
                .filter(|object_id| **object_id == source)
                .count(),
            1,
            "a replay cannot commit the sacrifice twice"
        );
        assert!(!engine.state.players[0].graveyard.contains(&source));
    }

    #[test]
    fn program_choose_sacrifice_cost_decline_and_empty_selection_use_otherwise_once() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        for explicit_decline in [false, true] {
            let (mut engine, top, owners, source, _borrowed) =
                controller_sacrifice_choose_fixture(true);
            let starting_life = engine.state.players[0].life;
            nested_start(
                &mut engine,
                &top,
                owners,
                crate::state::EffectResult::default(),
            );
            nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());

            let completed = if explicit_decline {
                let pending = engine.state.pending_resolution.as_ref().unwrap();
                let actor = pending.deciding_player;
                let command = RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            resolution_prompt_id: pending.resolution_prompt_id.get(),
                            decision: Decision::Decline as i32,
                            ..Default::default()
                        },
                    )),
                };
                engine
                    .apply_scenario_command(actor, &command)
                    .expect("an optional sacrifice cost can be declined")
            } else {
                nested_select_objects(&mut engine, Vec::new())
            };
            assert_eq!(engine.state.players[0].life, starting_life + 10);
            let life_deltas = completed
                .events
                .iter()
                .filter_map(|event| match &event.ev {
                    Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(life_deltas, vec![7, 3]);
            assert!(engine.state.objects[&source].zone == Zone::Battlefield);
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.state.stack.is_empty());
        }
    }

    #[test]
    fn program_choose_sacrifice_with_no_legal_candidate_enters_otherwise_once() {
        let (mut engine, top, owners, source, _borrowed) =
            controller_sacrifice_choose_fixture(true);
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .expect("the source leaves before its trigger resolves");
        let starting_life = engine.state.players[0].life;
        let run = engine
            .begin_program_run(&top, owners)
            .expect("the retained trigger Program starts from its stack object");
        let mut events = Vec::new();
        assert_eq!(
            engine
                .run_program_with_previous(
                    &top,
                    run,
                    crate::state::EffectResult::default(),
                    &mut events,
                )
                .expect("no legal sacrifice branch enters Otherwise and its parent tail"),
            ResolutionProgress::Completed
        );
        assert_eq!(engine.state.players[0].life, starting_life + 10);
        let life_deltas = events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(life_deltas, vec![7, 3]);
        assert!(!events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(_))
        )));
        assert!(engine.state.pending_resolution.is_none());
    }

    #[test]
    fn program_choose_sacrifice_payment_rejects_wrong_and_duplicate_objects_atomically() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let (mut engine, top, owners, source, borrowed) = controller_sacrifice_choose_fixture(true);
        let starting_life = engine.state.players[0].life;
        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        let prompt_id = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();

        let wrong_player = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: prompt_id,
                    chosen_object_ids: vec![source],
                    ..Default::default()
                },
            )),
        };
        assert!(engine.apply_scenario_command(1, &wrong_player).is_err());

        for chosen in [vec![borrowed], vec![source, source]] {
            let command = RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: prompt_id,
                        chosen_object_ids: chosen,
                        ..Default::default()
                    },
                )),
            };
            assert!(engine.apply_scenario_command(0, &command).is_err());
            assert_eq!(engine.state.players[0].life, starting_life);
            assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
            assert!(engine.state.pending_resolution.is_some());
        }
    }

    #[test]
    fn program_choose_sacrifice_payment_refreshes_returned_candidate_generation() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let (mut engine, top, owners, source, borrowed) =
            controller_sacrifice_choose_fixture(false);
        let starting_life = engine.state.players[0].life;
        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        let old_prompt_id = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();
        let old_generation = engine.state.zone_change_generation[&source];

        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .expect("the selected permanent leaves before payment");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            Some(0),
        )
        .expect("the same physical card returns as a new permanent incarnation");
        let current_generation = engine.state.zone_change_generation[&source];
        assert_ne!(old_generation, current_generation);

        let stale_answer = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: old_prompt_id,
                    chosen_object_ids: vec![source],
                    ..Default::default()
                },
            )),
        };
        assert!(engine.apply_scenario_command(0, &stale_answer).is_err());
        assert_eq!(engine.state.players[0].life, starting_life);
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
        assert!(engine.state.pending_resolution.is_some());

        let mut refreshed = Vec::new();
        engine
            .refresh_program_choose_departure(&mut refreshed)
            .expect("the payment offer refreshes against current object generations");
        engine
            .publish_resolution_prompt_ids(&mut refreshed)
            .expect("the changed generation gets a fresh prompt ID");
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_ne!(pending.resolution_prompt_id.get(), old_prompt_id);
        assert_eq!(pending.presentation.candidates.len(), 2);
        assert!(pending.presentation.candidates.contains(&source));
        assert!(pending.presentation.candidates.contains(&borrowed));
        let crate::state::ResolutionContinuation::ProgramChoose { choose, .. } =
            &pending.continuation
        else {
            panic!("the refreshed sacrifice occurrence stays parked");
        };
        let crate::state::PendingProgramChooseStage::PayingPermanentCost {
            candidate_generations,
            ..
        } = &choose.stage
        else {
            panic!("the refreshed payment retains its exact Program stage");
        };
        assert!(candidate_generations.contains(&(source, current_generation)));

        let paid = nested_select_objects(&mut engine, vec![source]);
        assert_eq!(engine.state.players[0].life, starting_life + 5);
        let life_deltas = paid
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(life_deltas, vec![2, 3]);
    }

    #[test]
    fn synthetic_program_choose_required_permanent_payment_restores_after_observer_return() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let (mut engine, top, owners, source, borrowed) =
            controller_sacrifice_choose_fixture_with_optionality(false, false);
        let starting_life = engine.state.players[0].life;
        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        let initial_payment = nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        let initial_offer = initial_payment
            .events
            .iter()
            .rev()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .expect("mandatory permanent-cost payment offer");
        assert_eq!((initial_offer.min, initial_offer.max), (1, 1));
        assert!(!initial_offer.prompt_text.contains("decline"));
        let original_prompt_id = initial_offer.resolution_prompt_id;
        let original_pending = engine
            .state
            .pending_resolution
            .take()
            .expect("the permanent-cost payment is saved during observer return");
        let crate::state::ResolutionContinuation::ProgramChoose { stack, choose } =
            &original_pending.continuation
        else {
            panic!("the saved payment retains its Program Choose continuation");
        };
        assert!(!choose.optional, "the fixture is a mandatory Choose");
        let original_program = format!("{:?}", stack.program);
        let old_generation = engine.state.zone_change_generation[&borrowed];
        let crate::state::PendingProgramChooseStage::PayingPermanentCost {
            candidate_generations: original_generations,
            ..
        } = &choose.stage
        else {
            panic!("the saved continuation retains its permanent-cost stage");
        };
        assert!(original_generations.contains(&(borrowed, old_generation)));
        let saved_generations = original_generations.to_vec();

        engine.state.pending_observer_return_batch =
            Some(crate::state::PendingObserverReturnBatch {
                ready: Vec::new(),
                remaining: Default::default(),
                resume_stack: None,
                suspended_resolution: Some(original_pending),
                suspended_replacement_event: None,
            });
        let observer = engine
            .state
            .pending_observer_return_batch
            .take()
            .expect("synthetic observer-return fixture");
        let mut restored = Vec::new();
        assert!(engine
            .restore_suspended_observer_resolution(
                observer.suspended_resolution,
                observer.suspended_replacement_event,
                &mut restored,
            )
            .expect("restore the saved permanent-cost offer"));
        engine
            .publish_resolution_prompt_ids(&mut restored)
            .expect("restoration publishes a fresh permanent-cost prompt");

        let restored_offer = restored
            .iter()
            .rev()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .expect("restored mandatory payment offer");
        assert_ne!(restored_offer.resolution_prompt_id, original_prompt_id);
        assert_eq!((restored_offer.min, restored_offer.max), (1, 1));
        assert_eq!(
            restored_offer.prompt_text,
            "Choose a permanent to sacrifice."
        );
        assert!(restored_offer.candidate_object_ids.contains(&source));
        assert!(restored_offer.candidate_object_ids.contains(&borrowed));
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("restored payment remains pending");
        assert_eq!(
            pending.resolution_prompt_id.get(),
            restored_offer.resolution_prompt_id
        );
        assert_eq!((pending.presentation.min, pending.presentation.max), (1, 1));
        assert_eq!(
            pending.presentation.prompt,
            "Choose a permanent to sacrifice."
        );
        let crate::state::ResolutionContinuation::ProgramChoose { stack, choose } =
            &pending.continuation
        else {
            panic!("restored payment retains its Program Choose continuation");
        };
        assert!(!choose.optional);
        assert_eq!(format!("{:?}", stack.program), original_program);
        let crate::state::PendingProgramChooseStage::PayingPermanentCost {
            candidate_generations,
            ..
        } = &choose.stage
        else {
            panic!("restored payment retains its permanent-cost stage");
        };
        assert_eq!(candidate_generations, &saved_generations);

        let decline = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: restored_offer.resolution_prompt_id,
                    decision: Decision::Decline as i32,
                    ..Default::default()
                },
            )),
        };
        let before_decline = command_snapshot(&engine);
        assert!(engine.apply_scenario_command(0, &decline).is_err());
        assert!(
            command_snapshot(&engine) == before_decline,
            "a mandatory permanent cost must reject Decline atomically"
        );
        let empty = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: restored_offer.resolution_prompt_id,
                    ..Default::default()
                },
            )),
        };
        assert!(engine.apply_scenario_command(0, &empty).is_err());
        assert!(
            command_snapshot(&engine) == before_decline,
            "a mandatory permanent cost must reject an empty payment atomically"
        );

        let paid = nested_select_objects(&mut engine, vec![borrowed]);
        assert_eq!(engine.state.players[0].life, starting_life + 5);
        let life_deltas = paid
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            life_deltas,
            vec![2, 3],
            "paid body and parent tail run once"
        );
        assert_eq!(engine.state.objects[&borrowed].zone, Zone::Graveyard);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
    }

    #[test]
    fn synthetic_program_choose_filtered_discard_payment_restores_same_candidates_after_observer_return(
    ) {
        use crate::state::PendingProgramChooseStage;
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let (mut engine, top, owners, _source, candidate) =
            controller_filtered_discard_choose_fixture();
        let starting_life = engine.state.players[0].life;
        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        let initial_payment = nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        let initial_offer = initial_payment
            .events
            .iter()
            .rev()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .expect("optional filtered-discard payment offer");
        assert_eq!(initial_offer.deciding_player_id, 0);
        assert_eq!(initial_offer.choice_kind(), rv1::ChoiceKind::HandCards);
        assert_eq!((initial_offer.min, initial_offer.max), (0, 1));
        assert_eq!(initial_offer.candidate_object_ids, vec![candidate]);
        let original_prompt_id = initial_offer.resolution_prompt_id;

        let original_pending = engine
            .state
            .pending_resolution
            .take()
            .expect("the discard payment is suspended for the synthetic observer return");
        assert_eq!(original_pending.deciding_player, 0);
        let ResolutionContinuation::ProgramChoose { stack, choose } =
            &original_pending.continuation
        else {
            panic!("saved filtered-discard payment keeps ProgramChoose");
        };
        assert!(choose.optional);
        let original_program = format!("{:?}", stack.program);
        let original_stack_id = stack.item.id;
        let original_controller = stack.item.controller;
        let original_owner = stack.item.source_owner;
        let generation = engine
            .state
            .zone_change_generation
            .get(&candidate)
            .copied()
            .unwrap_or(0);
        let PendingProgramChooseStage::PayingDiscardCardCost {
            candidate_generations,
            ..
        } = &choose.stage
        else {
            panic!("the saved fixture is in its filtered-discard payment stage");
        };
        let saved_generations = candidate_generations.to_vec();
        assert_eq!(saved_generations, vec![(candidate, generation)]);

        engine.state.pending_observer_return_batch =
            Some(crate::state::PendingObserverReturnBatch {
                ready: Vec::new(),
                remaining: Default::default(),
                resume_stack: None,
                suspended_resolution: Some(original_pending),
                suspended_replacement_event: None,
            });
        let observer = engine
            .state
            .pending_observer_return_batch
            .take()
            .expect("synthetic observer-return batch");
        let mut restored = Vec::new();
        assert!(engine
            .restore_suspended_observer_resolution(
                observer.suspended_resolution,
                observer.suspended_replacement_event,
                &mut restored,
            )
            .expect("restore the synthetic filtered-discard payment"));
        engine
            .publish_resolution_prompt_ids(&mut restored)
            .expect("restoration gives the saved offer a fresh prompt ID");
        let offers = restored
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(offers.len(), 1, "restoration republishes one private offer");
        let restored_offer = offers[0];
        assert_ne!(restored_offer.resolution_prompt_id, original_prompt_id);
        assert_eq!(restored_offer.deciding_player_id, 0);
        assert_eq!(restored_offer.choice_kind(), rv1::ChoiceKind::HandCards);
        assert_eq!((restored_offer.min, restored_offer.max), (0, 1));
        assert_eq!(restored_offer.candidate_object_ids, vec![candidate]);
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("restored payment remains pending for its original payer");
        assert_eq!(pending.deciding_player, 0);
        assert_eq!(
            pending.resolution_prompt_id.get(),
            restored_offer.resolution_prompt_id
        );
        assert_eq!(pending.presentation.candidates, vec![candidate]);
        let ResolutionContinuation::ProgramChoose { stack, choose } = &pending.continuation else {
            panic!("restored payment retains ProgramChoose");
        };
        assert!(choose.optional);
        assert_eq!(format!("{:?}", stack.program), original_program);
        assert_eq!(stack.item.id, original_stack_id);
        assert_eq!(stack.item.controller, original_controller);
        assert_eq!(stack.item.source_owner, original_owner);
        let PendingProgramChooseStage::PayingDiscardCardCost {
            candidate_generations,
            ..
        } = &choose.stage
        else {
            panic!("restoration retains the filtered-discard cost stage");
        };
        assert_eq!(candidate_generations, &saved_generations);

        let paid = nested_select_objects(&mut engine, vec![candidate]);
        assert_eq!(engine.state.players[0].life, starting_life + 5);
        let life_deltas = paid
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(life_deltas, vec![2, 3], "paid body and tail each run once");
        assert_eq!(engine.state.objects[&candidate].zone, Zone::Graveyard);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());

        let before_replay = command_snapshot(&engine);
        let replay = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: restored_offer.resolution_prompt_id,
                    chosen_object_ids: vec![candidate],
                    ..Default::default()
                },
            )),
        };
        assert!(engine.apply_scenario_command(0, &replay).is_err());
        assert!(
            command_snapshot(&engine) == before_replay,
            "stale prompt replay leaves engine state unchanged"
        );
    }

    #[test]
    fn synthetic_program_choose_filtered_discard_refreshes_returned_candidate_generation() {
        use crate::state::PendingProgramChooseStage;
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let (mut engine, top, owners, _source, candidate) =
            controller_filtered_discard_choose_fixture();
        let starting_life = engine.state.players[0].life;
        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        let initial_payment = nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        let initial_offer = initial_payment
            .events
            .iter()
            .rev()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .expect("the selected branch opens its filtered-discard payment");
        assert_eq!(initial_offer.deciding_player_id, 0);
        assert_eq!(initial_offer.choice_kind(), rv1::ChoiceKind::HandCards);
        assert_eq!((initial_offer.min, initial_offer.max), (0, 1));
        assert_eq!(initial_offer.candidate_object_ids, vec![candidate]);
        let old_prompt_id = initial_offer.resolution_prompt_id;
        let old_generation = engine
            .state
            .zone_change_generation
            .get(&candidate)
            .copied()
            .unwrap_or(0);
        let old_candidates = {
            let pending = engine.state.pending_resolution.as_ref().unwrap();
            let ResolutionContinuation::ProgramChoose { choose, .. } = &pending.continuation else {
                panic!("the original filtered-discard offer remains in ProgramChoose");
            };
            let PendingProgramChooseStage::PayingDiscardCardCost {
                candidate_generations,
                ..
            } = &choose.stage
            else {
                panic!("the original offer is in its discard-cost stage");
            };
            candidate_generations.clone()
        };
        assert_eq!(old_candidates, vec![(candidate, old_generation)]);

        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            candidate,
            Zone::Graveyard,
            None,
        )
        .expect("the same physical candidate leaves the payer's hand");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            candidate,
            Zone::Hand,
            Some(0),
        )
        .expect("the same physical candidate returns to the payer's hand");
        let current_generation = engine.state.zone_change_generation[&candidate];
        assert_ne!(old_generation, current_generation);

        let mut refreshed = Vec::new();
        engine
            .refresh_program_choose_departure(&mut refreshed)
            .expect("the explicit refresh rebinds the current filtered-hand candidate");
        engine
            .publish_resolution_prompt_ids(&mut refreshed)
            .expect("the changed candidate generation gets a fresh prompt ID");
        let offers = refreshed
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(offers.len(), 1);
        let refreshed_offer = offers[0];
        assert_ne!(refreshed_offer.resolution_prompt_id, old_prompt_id);
        assert_eq!(refreshed_offer.deciding_player_id, 0);
        assert_eq!(refreshed_offer.choice_kind(), rv1::ChoiceKind::HandCards);
        assert_eq!((refreshed_offer.min, refreshed_offer.max), (0, 1));
        assert_eq!(refreshed_offer.candidate_object_ids, vec![candidate]);
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(pending.deciding_player, 0);
        assert_eq!(pending.presentation.candidates, vec![candidate]);
        assert_eq!(
            pending.resolution_prompt_id.get(),
            refreshed_offer.resolution_prompt_id
        );
        let ResolutionContinuation::ProgramChoose { choose, .. } = &pending.continuation else {
            panic!("the refreshed offer stays in ProgramChoose");
        };
        let PendingProgramChooseStage::PayingDiscardCardCost {
            candidate_generations,
            ..
        } = &choose.stage
        else {
            panic!("the refreshed offer stays in its discard-cost stage");
        };
        assert_eq!(
            candidate_generations,
            &vec![(candidate, current_generation)]
        );

        let before_stale_replay = command_snapshot(&engine);
        let stale_replay = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: old_prompt_id,
                    chosen_object_ids: vec![candidate],
                    ..Default::default()
                },
            )),
        };
        assert!(engine.apply_scenario_command(0, &stale_replay).is_err());
        assert!(
            command_snapshot(&engine) == before_stale_replay,
            "the invalidated prompt cannot consume the rebound physical candidate"
        );

        let paid = nested_select_objects(&mut engine, vec![candidate]);
        assert_eq!(engine.state.players[0].life, starting_life + 5);
        let life_deltas = paid
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            life_deltas,
            vec![2, 3],
            "paid body and parent tail run once"
        );
        assert_eq!(engine.state.objects[&candidate].zone, Zone::Graveyard);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
    }

    #[test]
    fn program_choose_sacrifice_source_theft_routes_to_otherwise() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let (mut engine, top, owners, source, _borrowed) =
            controller_sacrifice_choose_fixture(true);
        let starting_life = engine.state.players[0].life;
        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .expect("the source leaves before payment");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            Some(1),
        )
        .expect("the source returns under the opponent's control");

        let mut refreshed = Vec::new();
        engine
            .refresh_program_choose_departure(&mut refreshed)
            .expect("a source-only cost loses its stolen source candidate");
        assert_eq!(engine.state.objects[&source].controller, 1);
        assert_eq!(engine.state.players[0].life, starting_life + 10);
        let life_deltas = refreshed
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(life_deltas, vec![7, 3]);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
    }

    #[test]
    fn program_choose_sacrifice_departure_drops_payer_but_not_surviving_payer_otherwise() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let (mut payer_engine, payer_top, payer_owners, payer_source, _payer_borrowed) =
            controller_sacrifice_choose_fixture(true);
        let payer_starting_life = payer_engine.state.players[0].life;
        nested_start(
            &mut payer_engine,
            &payer_top,
            payer_owners,
            crate::state::EffectResult::default(),
        );
        nested_answer(&mut payer_engine, Decision::SelectBranch, 0, Vec::new());
        payer_engine.state.players[0].has_lost = true;
        let mut payer_departure = Vec::new();
        payer_engine
            .refresh_program_choose_departure(&mut payer_departure)
            .expect("a departed Controller payer loses its owning resolution");
        assert!(payer_engine.state.pending_resolution.is_none());
        assert_eq!(payer_engine.state.players[0].life, payer_starting_life);
        assert_eq!(
            payer_engine.state.objects[&payer_source].zone,
            Zone::Battlefield
        );
        assert!(!payer_departure
            .iter()
            .any(|event| matches!(&event.ev, Some(rv1::ruled_event::Ev::LifeChanged(_)))));

        let (mut owner_engine, owner_top, owner_owners, owner_source, owner_borrowed) =
            controller_sacrifice_choose_fixture(true);
        let owner_starting_life = owner_engine.state.players[0].life;
        nested_start(
            &mut owner_engine,
            &owner_top,
            owner_owners,
            crate::state::EffectResult::default(),
        );
        nested_answer(&mut owner_engine, Decision::SelectBranch, 0, Vec::new());
        assert_eq!(owner_engine.state.objects[&owner_source].owner, 1);
        assert_eq!(owner_engine.state.objects[&owner_source].controller, 0);
        owner_engine.state.players[1].has_lost = true;
        for object_id in [owner_source, owner_borrowed] {
            if owner_engine.state.objects[&object_id].zone == Zone::Battlefield {
                move_object_to_zone(
                    &mut owner_engine.state,
                    owner_engine.registry,
                    object_id,
                    Zone::Exile,
                    None,
                )
                .expect("a departed owner's permanent leaves the game");
            }
        }
        let mut owner_departure = Vec::new();
        owner_engine
            .refresh_program_choose_departure(&mut owner_departure)
            .expect("the surviving payer's candidates are pruned after owner departure");
        assert_eq!(owner_engine.state.players[0].life, owner_starting_life + 10);
        let life_deltas = owner_departure
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(life_deltas, vec![7, 3]);
        assert!(owner_engine.state.pending_resolution.is_none());
        assert_eq!(owner_engine.state.objects[&owner_source].owner, 1);
        assert_eq!(owner_engine.state.objects[&owner_source].zone, Zone::Exile);
        assert_eq!(owner_engine.state.objects[&owner_source].controller, 1);
        assert!(!owner_engine.state.players[0]
            .graveyard
            .contains(&owner_source));
        assert!(!owner_departure.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::Log(log)) if log.text.starts_with("P0 sacrifices ")
        )));
    }

    #[test]
    fn program_choose_sacrifice_prunes_only_departed_owner_candidates_and_reoffers() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let (mut engine, top, owners, source, borrowed) =
            controller_sacrifice_choose_fixture(false);
        let own_candidate = *engine.state.players[0]
            .hand
            .iter()
            .find(|object_id| {
                engine.state.objects[object_id].card_id == "controller_sacrifice_program_fixture"
            })
            .expect("player zero owns another eligible permanent");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            own_candidate,
            Zone::Battlefield,
            Some(0),
        )
        .expect("player zero's own permanent enters under player zero's control");
        let starting_life = engine.state.players[0].life;
        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());

        let old_prompt_id = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("sacrifice payment is offered")
            .resolution_prompt_id
            .get();
        let offered = &engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates;
        assert_eq!(offered.len(), 3);
        assert!(offered.contains(&source));
        assert!(offered.contains(&borrowed));
        assert!(offered.contains(&own_candidate));
        assert_eq!(engine.state.objects[&source].owner, 1);
        assert_eq!(engine.state.objects[&source].controller, 0);
        assert_eq!(engine.state.objects[&borrowed].owner, 1);
        assert_eq!(engine.state.objects[&borrowed].controller, 0);
        assert_eq!(engine.state.objects[&own_candidate].owner, 0);
        assert_eq!(engine.state.objects[&own_candidate].controller, 0);

        engine.state.players[1].has_lost = true;
        for object_id in [source, borrowed] {
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                object_id,
                Zone::Exile,
                None,
            )
            .expect("a departed owner's permanent leaves the game");
        }
        let mut departure = Vec::new();
        engine
            .refresh_program_choose_departure(&mut departure)
            .expect(
                "the surviving payer keeps the chooser and loses only departed-owner candidates",
            );
        engine
            .publish_resolution_prompt_ids(&mut departure)
            .expect("the narrowed offer receives a new prompt ID");

        assert!(!engine.state.players[0].has_lost);
        assert!(engine.state.players[1].has_lost);
        assert!(!engine.state.players[2].has_lost);
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("player zero receives the surviving candidate");
        assert_eq!(pending.deciding_player, 0);
        assert_ne!(pending.resolution_prompt_id.get(), old_prompt_id);
        assert_eq!(pending.presentation.candidates, vec![own_candidate]);

        let stale_answer = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: old_prompt_id,
                    chosen_object_ids: vec![own_candidate],
                    ..Default::default()
                },
            )),
        };
        assert!(engine.apply_scenario_command(0, &stale_answer).is_err());
        assert_eq!(engine.state.players[0].life, starting_life);
        assert!(engine.state.pending_resolution.is_some());

        assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
        assert_eq!(engine.state.objects[&borrowed].zone, Zone::Exile);
        let paid = nested_select_objects(&mut engine, vec![own_candidate]);
        assert_eq!(engine.state.players[0].life, starting_life + 5);
        assert_eq!(engine.state.objects[&own_candidate].zone, Zone::Graveyard);
        assert!(!engine.state.objects.contains_key(&source));
        assert!(!engine.state.objects.contains_key(&borrowed));
        assert!(engine.state.pending_resolution.is_none());
        let life_deltas = paid
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::LifeChanged(change)) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(life_deltas, vec![2, 3]);
        assert_eq!(
            paid.events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::Log(log))
                        if log.text.starts_with("P0 sacrifices ")
                ))
                .count(),
            1,
            "the surviving candidate is paid once"
        );
        assert!(!paid.events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                if moved.object_id == source || moved.object_id == borrowed
        )));
        assert_eq!(
            paid.events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                        if moved.object_id == own_candidate
                            && moved.destination
                                == rv1::permanent_moved::Destination::Graveyard as i32
                ))
                .count(),
            1,
            "the one sacrifice log corresponds to the surviving payer's chosen physical object"
        );
        assert!(!engine.state.players[0].has_lost);
        assert!(!engine.state.players[2].has_lost);
    }

    fn source_controller_paid_choose_fixture() -> (
        GameEngine,
        StackItem,
        Vec<tricerules_card_model::ProgramOwnerId>,
    ) {
        use crate::state::{TriggerAbilityOrigin, TriggerUseKey};

        let card = r#"(
            id: "source_controller_paid_fixture",
            name: "Source Controller Paid Fixture",
            face_id: "source_controller_paid_fixture",
            types: ["Creature"], power: 2, toughness: 2,
            triggered_abilities: [(
                ability_id: "triggered_01", presentation: Fallback,
                trigger: WhenSelfEntersBattlefield, effect: [],
                program: Some((body: (steps: [
                    (id: "choose", kind: Choose(
                        choice: "paid", chooser: SourceController, optional: false,
                        branches: [
                            (branch_id: "mana", presentation: Fallback, cost: Mana("{1}"),
                                requirement: Always,
                                body: (steps: [(id: "mana_gain", kind: Perform(GainLife(amount: 1)))], yields: [])),
                            (branch_id: "waterbend", presentation: Fallback, cost: Waterbend("{1}"),
                                requirement: Always,
                                body: (steps: [(id: "waterbend_gain", kind: Perform(GainLife(amount: 2)))], yields: [])),
                        ],
                        otherwise: (steps: [(id: "otherwise_gain", kind: Perform(GainLife(amount: 3)))], yields: []),
                    ))
                ], yields: []), target_bindings: [])),
            )],
        )"#;
        let card_id = "source_controller_paid_fixture";
        let registry = Box::leak(Box::new(
            tricerules_card_model::CardRegistry::from_chunks_and_tokens(&[card], &[])
                .expect("SourceController paid Choose fixture compiles"),
        ));
        let decks = Some((0..3).map(|_| vec![card_id.to_string(); 30]).collect());
        let mut engine = GameEngine::new(registry, 91_207, &[0, 1, 2], 20, decks, true)
            .expect("three-player SourceController fixture");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;

        let source = *engine.state.players[0]
            .hand
            .iter()
            .find(|object_id| engine.state.objects[object_id].card_id == card_id)
            .expect("fixture source is in player zero's hand");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            Some(1),
        )
        .expect("source is controlled by player one when the trigger resolves");

        let face = registry
            .get(card_id)
            .and_then(|definition| definition.face(0))
            .expect("fixture face");
        let ability = face
            .triggered_abilities
            .first()
            .expect("fixture triggered ability")
            .clone();
        let definition = tricerules_card_model::AbilityDefinitionId {
            card_id: card_id.into(),
            face_id: face.face_id.clone(),
            ability_path: vec![ability.ability_id.clone()],
        };
        let generation = engine.state.zone_change_generation[&source];
        let mut top = engine.observer_return_item(source, 0);
        top.id = engine.state.next_object_id.saturating_add(1);
        top.controller = 0;
        top.card_id = card_id.into();
        top.ability_text = Some("Source Controller Paid Fixture trigger".into());
        top.source_permanent_id = Some(source);
        top.source_owner = Some(0);
        top.source_zone_change = generation;
        top.ability_index = Some(0);
        top.triggered_ability = Some(ability);
        top.trigger_use_key = Some(TriggerUseKey {
            object_id: source,
            zone_change_generation: generation,
            ability_origin: TriggerAbilityOrigin::Printed(definition),
        });
        top.is_triggered = true;
        top.is_copy = false;
        let owners = engine
            .compiled_resolution_program_owners(&top)
            .expect("trigger program owner lookup")
            .expect("fixture ability has a compiled program");
        engine.state.stack.push(top.clone());
        (engine, top, owners)
    }

    #[test]
    fn source_controller_paid_choose_uses_source_controller_for_mana_and_waterbend() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        for (branch, waterbend, expected_life) in [(0, false, 21), (1, true, 22)] {
            let (mut engine, top, owners) = source_controller_paid_choose_fixture();
            engine.state.players[0].mana_pool.colorless = 1;
            engine.state.players[1].mana_pool.colorless = 1;

            nested_start(
                &mut engine,
                &top,
                owners,
                crate::state::EffectResult::default(),
            );
            let (pending, _, _) = parked_program(&engine);
            assert_eq!(top.controller, 0);
            assert_eq!(
                pending.deciding_player, 1,
                "current source controller chooses"
            );
            let choice = nested_answer(&mut engine, Decision::SelectBranch, branch, Vec::new());
            assert_eq!(
                nested_log_count(&choice, "P1 chooses: Pay {1}."),
                usize::from(branch == 0)
            );
            assert_eq!(
                nested_log_count(&choice, "P1 chooses: Waterbend {1}."),
                usize::from(branch == 1)
            );

            let pending = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("payment is pending");
            assert_eq!(pending.deciding_player, 1, "the chooser remains the payer");
            let ResolutionContinuation::ProgramChoose { choose, .. } = &pending.continuation else {
                panic!("paid SourceController Choose remains parked");
            };
            assert_eq!(choose.original_chooser, 1);
            let payment = pending
                .continuation
                .mana_payment()
                .expect("selected branch has a payment");
            assert_eq!(payment.waterbend, waterbend);

            let paid = nested_answer(&mut engine, Decision::PayMana, 0, Vec::new());
            assert_eq!(nested_log_count(&paid, "P1 pays {1} during resolution."), 1);
            assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
            assert_eq!(engine.state.players[1].mana_pool.colorless, 0);
            assert_eq!(engine.state.players[0].life, expected_life);
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.state.stack.is_empty());
        }
    }

    #[test]
    fn source_controller_departure_during_paid_choose_uses_captured_original_chooser() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let (mut engine, top, owners) = source_controller_paid_choose_fixture();
        engine.state.players[0].mana_pool.colorless = 1;
        engine.state.players[1].mana_pool.colorless = 1;
        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("payment is pending");
        assert_eq!(pending.deciding_player, 1);
        let ResolutionContinuation::ProgramChoose { choose, .. } = &pending.continuation else {
            panic!("paid SourceController Choose remains parked");
        };
        assert_eq!(choose.original_chooser, 1);

        engine.state.players[1].has_lost = true;
        let mut departure_events = Vec::new();
        engine
            .refresh_program_choose_departure(&mut departure_events)
            .expect("departed payer resolves Otherwise");

        assert!(departure_events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::Log(log))
                if log.text == "The departed payer cannot complete the program Choose payment; Otherwise resolves."
        )));
        assert_eq!(
            engine.state.players[0].life, 23,
            "only Otherwise resolves for the live stack controller"
        );
        assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
        assert_eq!(engine.state.players[1].mana_pool.colorless, 1);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
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

    fn nested_direct_top_fixture(
        card: &str,
        card_id: &str,
        seed: u64,
        with_forest: bool,
        empty_library: bool,
    ) -> (
        GameEngine,
        StackItem,
        Vec<tricerules_card_model::ProgramOwnerId>,
        Option<ObjectId>,
        ObjectId,
    ) {
        assert!(!(with_forest && empty_library));
        let registry = Box::leak(Box::new(
            tricerules_card_model::CardRegistry::from_chunks_and_tokens(
                &[
                    card,
                    include_str!("../../../../tricerules-cards/data/island.ron"),
                    include_str!("../../../../tricerules-cards/data/forest.ron"),
                    include_str!("../../../../tricerules-cards/data/wan_shi_tong,_librarian.ron"),
                ],
                &[],
            )
            .expect("direct TopOfLibrary fixture registry"),
        ));
        let mut controller_deck = vec![card_id.to_string(); 30];
        if with_forest {
            controller_deck[0] = "forest".into();
        }
        let mut opponent_deck = vec!["wan_shi_tong,_librarian".to_string()];
        opponent_deck.extend(std::iter::repeat_n("island".to_string(), 29));
        let decks = Some(vec![controller_deck, opponent_deck]);
        let mut engine = GameEngine::new(registry, seed, &[0, 1], 20, decks, true)
            .expect("direct TopOfLibrary fixture engine");
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::Main1;
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;

        let forest = if with_forest {
            let object_id = *engine
                .state
                .objects
                .iter()
                .find_map(|(object_id, object)| (object.card_id == "forest").then_some(object_id))
                .expect("one fixture Forest exists");
            if engine.state.objects[&object_id].zone != Zone::Library {
                move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    object_id,
                    Zone::Library,
                    Some(0),
                )
                .expect("fixture Forest returns to its owner's library");
            }
            Some(object_id)
        } else {
            None
        };

        let wan = *engine
            .state
            .objects
            .iter()
            .find_map(|(object_id, object)| {
                (object.card_id == "wan_shi_tong,_librarian").then_some(object_id)
            })
            .expect("opponent Wan Shi Tong exists");
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            wan,
            Zone::Battlefield,
            Some(1),
        )
        .expect("Wan enters the fixture battlefield without firing its ETB ability");

        let spell = *engine.state.players[0]
            .hand
            .iter()
            .find(|object_id| engine.state.objects[object_id].card_id == card_id)
            .expect("fixture spell in controller's hand");
        move_object_to_zone(&mut engine.state, engine.registry, spell, Zone::Stack, None)
            .expect("fixture spell moves to stack");
        if empty_library {
            for object_id in engine.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>()
            {
                move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    object_id,
                    Zone::Graveyard,
                    None,
                )
                .expect("empty-library fixture removes each card through a zone move");
            }
        }
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
        let owners = engine
            .compiled_resolution_program_owners(&top)
            .expect("direct search program owner lookup")
            .expect("direct search program owner exists");
        engine.state.stack.push(top.clone());
        (engine, top, owners, forest, wan)
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
    ) -> tricerules_proto::ruled::v1::RuledEventBatch {
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
            .expect("resolution choice accepted")
    }

    fn nested_select_objects(
        engine: &mut GameEngine,
        chosen: Vec<ObjectId>,
    ) -> tricerules_proto::ruled::v1::RuledEventBatch {
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
            .expect("object choice accepted")
    }

    #[test]
    fn program_damage_player_replacement_resumes_parent_tail_once() {
        use crate::state::{
            ActiveDamagePrevention, DamagePreventionAmount, DamagePreventionScope,
            ReplacementSourcePresentation,
        };
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let (mut engine, top, owners, target, own_graveyard, ..) =
            exile_group_input_fixture("program_exile_group_input_fixture", &[0], true);
        let own_graveyard = own_graveyard.unwrap();
        let original_generation = engine.state.zone_change_generation[&own_graveyard];
        let starting_parent_life = engine.state.players[0].life;
        let prevention_id = engine.state.next_damage_prevention_effect_id;
        engine
            .state
            .damage_prevention_effects
            .push(ActiveDamagePrevention {
                id: prevention_id,
                static_origin: None,
                source_id: None,
                source_label: "test prevention on the target controller".into(),
                source_presentation: ReplacementSourcePresentation::default(),
                scope: DamagePreventionScope::Recipient(30),
                amount: DamagePreventionAmount::All,
                duration: tricerules_card_model::primitives::EffectDuration::UntilEndOfTurn,
                additional_effect: None,
            });
        let second_prevention_id = prevention_id.saturating_add(1);
        assert_ne!(prevention_id, second_prevention_id);
        engine
            .state
            .damage_prevention_effects
            .push(ActiveDamagePrevention {
                id: second_prevention_id,
                static_origin: None,
                source_id: None,
                source_label: "another test prevention on the target controller".into(),
                source_presentation: ReplacementSourcePresentation::default(),
                scope: DamagePreventionScope::Recipient(30),
                amount: DamagePreventionAmount::All,
                duration: tricerules_card_model::primitives::EffectDuration::UntilEndOfTurn,
                additional_effect: None,
            });
        engine.state.next_damage_prevention_effect_id = second_prevention_id.saturating_add(1);

        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        assert_eq!(engine.state.objects[&target].damage, 1);
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            10
        );
        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .choice_kind,
            rv1::ChoiceKind::GraveyardCards
        );

        let paid = nested_select_objects(&mut engine, vec![own_graveyard]);
        assert_eq!(engine.state.objects[&target].damage, 1);
        assert_eq!(engine.state.objects[&own_graveyard].zone, Zone::Exile);
        assert_eq!(
            engine.state.zone_change_generation[&own_graveyard],
            original_generation + 1,
            "the branch cost commits exactly once before the late damage replacement"
        );
        assert_eq!(engine.state.players[0].life, starting_parent_life);
        let moved_cost_count = paid
            .events
            .iter()
            .filter(|event| {
                matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::PermanentMoved(moved))
                        if moved.object_id == own_graveyard
                            && moved.destination == rv1::permanent_moved::Destination::Exile as i32
                )
            })
            .count();
        assert_eq!(moved_cost_count, 1);

        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("late DamagePlayer parks on the target controller's prevention");
        assert_eq!(pending.deciding_player, 30);
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::ReplacementEffect
        );
        let parked_stack = match &pending.continuation {
            ResolutionContinuation::DamageReplacement { stack, .. } => stack,
            other => panic!("expected Program-carrying damage replacement, got {other:?}"),
        };
        assert!(
            parked_stack.program.is_some(),
            "the late DamagePlayer replacement retains the active Program and parent cursor"
        );
        let replacement_prompt = paid
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice))
                    if choice.choice_kind == rv1::ChoiceKind::ReplacementEffect as i32 =>
                {
                    Some(choice)
                }
                _ => None,
            })
            .expect("the prevention option is published with the replacement prompt");
        assert_eq!(replacement_prompt.replacement_options.len(), 2);
        assert_ne!(
            replacement_prompt.replacement_options[0].application_id,
            replacement_prompt.replacement_options[1].application_id,
            "the ordering prompt offers the two distinct prevention effects"
        );
        let replacement_id = replacement_prompt.replacement_options[0].application_id;

        let resumed = nested_select_objects(&mut engine, vec![replacement_id]);
        let tail_life_changes = resumed
            .events
            .iter()
            .filter(|event| {
                matches!(
                    &event.ev,
                    Some(rv1::ruled_event::Ev::LifeChanged(change)) if change.delta == 3
                )
            })
            .count();
        assert_eq!(
            tail_life_changes, 1,
            "the parent tail resolves exactly once"
        );
        assert!(!resumed.events.iter().any(|event| matches!(
            &event.ev,
            Some(rv1::ruled_event::Ev::LifeChanged(change)) if change.delta == -2
        )));
        assert_eq!(engine.state.players[0].life, starting_parent_life + 3);
        assert_eq!(
            engine.state.players[2].life, 20,
            "all target-controller damage is prevented"
        );
        assert_eq!(
            engine.state.objects[&target].damage, 1,
            "the earlier damage is not repeated"
        );
        assert_eq!(engine.state.objects[&own_graveyard].zone, Zone::Exile);
        assert_eq!(
            engine.state.zone_change_generation[&own_graveyard],
            original_generation + 1
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.objects[&top.id].zone, Zone::Graveyard);
    }

    fn nested_log_count(batch: &tricerules_proto::ruled::v1::RuledEventBatch, text: &str) -> usize {
        batch
            .events
            .iter()
            .filter(|event| {
                matches!(&event.ev, Some(rv1::ruled_event::Ev::Log(log)) if log.text == text)
            })
            .count()
    }

    fn nested_search_previous_result(
        engine: &GameEngine,
        wan: ObjectId,
    ) -> crate::state::EffectResult {
        let mut previous = crate::state::EffectResult::default();
        previous
            .produced_objects
            .push(crate::state::TriggerObjectRef {
                object_id: wan,
                zone_change_generation: engine.state.zone_change_generation[&wan],
                controller_at_event: 1,
            });
        previous
    }

    fn finish_one_wan_search_trigger(
        engine: &mut GameEngine,
        wan: ObjectId,
        player_one_hand_before: usize,
        wan_counters_before: u32,
    ) {
        assert_eq!(
            engine.state.stack.len(),
            1,
            "exactly one Wan search trigger is stacked"
        );
        assert_eq!(engine.state.stack[0].card_id, "wan_shi_tong,_librarian");
        assert_eq!(
            engine.state.stack[0]
                .triggered_ability
                .as_ref()
                .map(|ability| ability.ability_id.as_str()),
            Some("triggered_02")
        );
        engine
            .apply_scenario_command(0, &pass_priority())
            .expect("active player passes to Wan's search trigger");
        engine
            .apply_scenario_command(1, &pass_priority())
            .expect("opponent passes to resolve Wan's search trigger");
        assert_eq!(engine.state.stack.len(), 0);
        assert_eq!(
            engine.state.objects[&wan].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
            wan_counters_before + 1
        );
        assert_eq!(
            engine.state.players[1].hand.len(),
            player_one_hand_before + 1
        );
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
            | ResolutionContinuation::ProgramHandLibraryReturn { stack, .. }
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

    const HAND_LIBRARY_RETURN_WITH_TAIL: &str = r#"(id: "hand_library_return_tail_fixture",
 name: "Hand Library Return Tail Fixture", face_id: "hand_library_return_tail_fixture",
 mana_cost: "{0}", types: ["Instant"], spell_program: Some((body: (steps: [
  (id: "select", bind: ["selected"], input: None,
   kind: Perform(SelectHandCards(count: 2, ordered: true, prompt: "Choose two cards to put back."))),
  (id: "put_back", bind: [], input: Some("selected"),
   kind: Perform(PutSelectedHandCardsOnLibraryTop)),
  (id: "tail_draw", bind: ["drawn"], input: None,
   kind: Perform(Draw(who: Controller, count: 1)))
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
   optional: true, branches: [(branch_id: "continue", presentation: Fallback, cost: None,
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

    #[test]
    fn hand_library_return_resumes_the_program_tail_once_with_deterministic_replay() {
        use prost::Message;
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let run = || {
            let (mut engine, top, owners, _) = nested_choose_fixture(
                HAND_LIBRARY_RETURN_WITH_TAIL,
                "hand_library_return_tail_fixture",
                91_231,
                0,
                &[],
            );
            // This unit fixture marks a synthetic spell object as declared commander solely to
            // exercise the replacement and parked Program continuation. Registered identities
            // and Commander-deck legality remain covered by the scenario tests.
            let commander = *engine.state.players[0]
                .library
                .back()
                .expect("a synthetic card remains in the caster's library");
            move_object_to_zone(
                &mut engine.state,
                engine.registry,
                commander,
                Zone::Hand,
                None,
            )
            .expect("fixture object moves to hand");
            engine.state.players[0]
                .declared_commander_object_ids
                .push(commander);
            let other = *engine.state.players[0]
                .hand
                .iter()
                .find(|object_id| **object_id != commander)
                .expect("another card is available for ordered selection");
            let library_before = engine.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>();
            let before_draws = engine.state.turn_history.current.player(0).cards_drawn;

            nested_start(
                &mut engine,
                &top,
                owners,
                crate::state::EffectResult::default(),
            );
            let hand_prompt = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("hand selection pending");
            assert!(hand_prompt.presentation.candidates.contains(&commander));
            nested_select_objects(&mut engine, vec![commander, other]);
            let replacement = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("commander replacement pending");
            assert!(matches!(
                &replacement.continuation,
                ResolutionContinuation::ProgramHandLibraryReturn { .. }
            ));
            assert_eq!(
                engine.state.players[0]
                    .library
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
                library_before,
                "the Program tail remains parked before the replacement commits"
            );

            let completed = nested_answer(&mut engine, Decision::SelectBranch, 1, Vec::new());
            assert_eq!(
                engine.state.turn_history.current.player(0).cards_drawn,
                before_draws + 1,
                "the Program tail draw resumes exactly once"
            );
            assert_eq!(engine.state.objects[&commander].zone, Zone::Library);
            assert_eq!(engine.state.objects[&other].zone, Zone::Hand);
            assert_eq!(engine.state.players[0].library.front(), Some(&commander));
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.state.stack.is_empty());
            assert!(engine.state.players[0].graveyard.contains(&top.id));
            (
                completed.encode_to_vec(),
                engine.state.players[0].hand.clone(),
                engine.state.players[0]
                    .library
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
                engine.state.turn_history.current.player(0).cards_drawn,
            )
        };

        assert_eq!(
            run(),
            run(),
            "same seed and accepted answers replay identically"
        );
    }

    #[test]
    fn hand_library_return_survives_source_owner_departure_then_resumes_tail_once() {
        use prost::Message;
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;

        let run = || {
            let registry = Box::leak(Box::new(
                tricerules_card_model::CardRegistry::from_chunks_and_tokens(
                    &[
                        HAND_LIBRARY_RETURN_WITH_TAIL,
                        include_str!("../../../../tricerules-cards/data/island.ron"),
                        include_str!("../../../../tricerules-cards/data/forest.ron"),
                    ],
                    &[],
                )
                .expect("source-owner departure tail fixture registry"),
            ));
            let decks = Some(vec![
                vec!["hand_library_return_tail_fixture".to_string(); 30],
                vec!["island".to_string(); 30],
                vec!["forest".to_string(); 30],
            ]);
            let mut engine = GameEngine::new(registry, 91_232, &[0, 1, 2], 20, decks, true)
                .expect("three-player source-owner departure fixture engine");
            engine.state.opening = None;
            engine.state.turn_step = TurnStep::Main1;
            engine.state.active_player_idx = 0;
            engine.state.priority_idx = 1;

            // This parked-state fixture directly models a P0-owned physical source with P1 as
            // controller/caster/cohort owner. It tests continuation behavior after source-owner
            // departure, not a cast or control-change path exposed by a card API.
            let spell = *engine.state.players[0]
                .hand
                .first()
                .expect("P0 has a physical source card");
            move_object_to_zone(&mut engine.state, engine.registry, spell, Zone::Stack, None)
                .expect("physical source enters the modeled stack state");
            let generation = engine
                .state
                .zone_change_generation
                .get(&spell)
                .copied()
                .unwrap_or(0);
            let mut top = engine.observer_return_item(spell, 1);
            top.id = spell;
            top.controller = 1;
            top.card_id = "hand_library_return_tail_fixture".into();
            top.ability_text = None;
            top.is_copy = false;
            top.face_index = 0;
            top.source_owner = Some(0);
            top.cast_by = Some(1);
            top.cast_occurrence = Some(crate::state::StackObjectRef {
                object_id: spell,
                zone_change_generation: Some(generation),
            });
            let owners = engine
                .compiled_resolution_program_owners(&top)
                .expect("program owner lookup")
                .expect("fixture Program owner exists");
            engine.state.stack.push(top.clone());

            // Mark one exact P1-owned card as a fixture commander, then retain two exact hand
            // references in a deliberate order through the P0 source-owner departure.
            let commander = *engine.state.players[1]
                .hand
                .first()
                .expect("P1 has a cohort card");
            engine.state.players[1]
                .declared_commander_object_ids
                .push(commander);
            let other = *engine.state.players[1]
                .hand
                .iter()
                .find(|object_id| **object_id != commander)
                .expect("P1 has a second cohort card");
            let selected = [commander, other];
            let generations = selected
                .iter()
                .map(|object_id| {
                    engine
                        .state
                        .zone_change_generation
                        .get(object_id)
                        .copied()
                        .unwrap_or(0)
                })
                .collect::<Vec<_>>();
            let library_before = engine.state.players[1]
                .library
                .iter()
                .copied()
                .collect::<Vec<_>>();
            let before_draws = engine.state.turn_history.current.player(1).cards_drawn;

            nested_start(
                &mut engine,
                &top,
                owners,
                crate::state::EffectResult::default(),
            );
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .expect("P1 hand selection pending")
                    .deciding_player,
                1
            );
            nested_select_objects(&mut engine, selected.to_vec());
            let pending_before = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("P1 replacement choice pending");
            let continuation_before = format!("{:?}", pending_before.continuation);
            let prompt_id = pending_before.resolution_prompt_id.get();
            assert!(matches!(
                &pending_before.continuation,
                ResolutionContinuation::ProgramHandLibraryReturn { .. }
            ));

            let departure = engine
                .apply_scenario_command(
                    0,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                    },
                )
                .expect("physical source owner P0 leaves while P1's Program is parked");
            assert!(engine.state.players[0].has_lost);
            assert_eq!(
                engine
                    .state
                    .players
                    .iter()
                    .filter(|player| !player.has_lost)
                    .count(),
                2
            );
            assert!(engine.state.winner().is_none());
            assert!(!engine.state.objects.contains_key(&spell));
            assert_eq!(
                departure
                    .events
                    .iter()
                    .filter(|event| matches!(
                        &event.ev,
                        Some(rv1::ruled_event::Ev::StackObjectRemoved(removed))
                            if removed.object_id == spell
                    ))
                    .count(),
                1
            );
            let refreshed = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("P1's exact hand return survives source-owner departure");
            assert_eq!(refreshed.deciding_player, 1);
            assert_eq!(refreshed.resolution_prompt_id.get(), prompt_id);
            assert_eq!(
                format!("{:?}", refreshed.continuation),
                continuation_before,
                "selected order, generation-bound cohort, and Program frame are retained"
            );
            let refreshed_offer = departure.events.iter().find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            });
            let refreshed_offer = refreshed_offer.expect("P1's private offer is re-emitted");
            assert_eq!(refreshed_offer.deciding_player_id, 1);
            assert_eq!(
                refreshed_offer.choice_kind(),
                rv1::ChoiceKind::ResolutionBranch
            );
            assert!(refreshed_offer.candidate_object_ids.is_empty());
            assert!(refreshed_offer.candidate_names.is_empty());
            assert!(refreshed_offer.public_reveal.is_none());
            assert_eq!(
                refreshed_offer
                    .resolution_branches
                    .iter()
                    .map(|branch| branch.label.as_str())
                    .collect::<Vec<_>>(),
                vec!["Command zone", "Library"]
            );
            assert!(selected.iter().all(|object_id| {
                engine.state.players[1].hand.contains(object_id)
                    && engine.state.objects[object_id].zone == Zone::Hand
            }));
            assert_eq!(
                selected
                    .iter()
                    .map(|object_id| {
                        engine
                            .state
                            .zone_change_generation
                            .get(object_id)
                            .copied()
                            .unwrap_or(0)
                    })
                    .collect::<Vec<_>>(),
                generations
            );
            assert_eq!(
                engine.state.players[1]
                    .library
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
                library_before
            );

            let completed = nested_answer(&mut engine, Decision::SelectBranch, 1, Vec::new());
            assert_eq!(
                engine.state.turn_history.current.player(1).cards_drawn,
                before_draws + 1,
                "the resumed Program tail executes once after source-owner departure"
            );
            assert_eq!(engine.state.objects[&commander].zone, Zone::Library);
            assert_eq!(engine.state.objects[&other].zone, Zone::Hand);
            assert_eq!(engine.state.players[1].library.front(), Some(&commander));
            assert!(!engine.state.objects.contains_key(&spell));
            assert!(engine.state.pending_resolution.is_none());
            assert!(engine.state.stack.is_empty());
            (
                departure.encode_to_vec(),
                completed.encode_to_vec(),
                engine.state.players[1].hand.clone(),
                engine.state.players[1]
                    .library
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
                engine.state.turn_history.current.player(1).cards_drawn,
            )
        };

        assert_eq!(
            run(),
            run(),
            "same three-player fixture and accepted commands replay identically"
        );
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
       optional: true, branches: [(branch_id: "continue", presentation: Fallback, cost: None,
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

    const DIRECT_TOP_CHOOSE_DRAW: &str = r#"(id: "direct_top_choose_draw_fixture",
 name: "Direct Top Choose Draw Fixture", face_id: "direct_top_choose_draw_fixture",
 mana_cost: "{0}", types: ["Instant"], spell_program: Some((body: (steps: [
  (id: "search_choice", bind: [], input: None,
   kind: Choose(choice: "search", chooser: Controller, optional: true,
    branches: [(branch_id: "search", presentation: Fallback, cost: None,
     requirement: Always, body: (steps: [
      (id: "search_top", bind: [], input: None, kind: Perform(SearchLibrary(
       who: Controller, optional: false, count: 1, count_by_cast_cost: None,
       filter: Some((card_type: Some(BasicLand))), selection_constraint: None,
       slots: [], zones: Fixed([Library]), destination: TopOfLibrary,
       conditional_destination: None, shuffle: true, reveal: true, result_id: None,
       selection_bounds: Some((min: 0, max: 1)),
      )))
     ], yields: []))], otherwise: (steps: [], yields: []))),
  (id: "draw_top", bind: ["drawn"], input: None,
   kind: Perform(Draw(who: Controller, count: 1))),
  (id: "pause_after_draw", bind: [], input: None,
   kind: Choose(choice: "after_draw", chooser: Controller, optional: true,
    branches: [(branch_id: "finish", presentation: Fallback, cost: None,
     requirement: Always, body: (steps: [], yields: []))], otherwise: (steps: [], yields: [])))
 ], yields: []), target_bindings: [])))"#;

    const DIRECT_TOP_CHOOSE_SAFE_TAIL: &str = r#"(id: "direct_top_choose_safe_tail_fixture",
 name: "Direct Top Choose Safe Tail Fixture", face_id: "direct_top_choose_safe_tail_fixture",
 mana_cost: "{0}", types: ["Instant"], spell_program: Some((body: (steps: [
  (id: "search_choice", bind: [], input: None,
   kind: Choose(choice: "search", chooser: Controller, optional: true,
    branches: [(branch_id: "search", presentation: Fallback, cost: None,
     requirement: Always, body: (steps: [
      (id: "search_top", bind: [], input: None, kind: Perform(SearchLibrary(
       who: Controller, optional: false, count: 1, count_by_cast_cost: None,
       filter: Some((card_type: Some(BasicLand))), selection_constraint: None,
       slots: [], zones: Fixed([Library]), destination: TopOfLibrary,
       conditional_destination: None, shuffle: true, reveal: true, result_id: None,
       selection_bounds: Some((min: 0, max: 1)),
      )))
     ], yields: []))], otherwise: (steps: [], yields: []))),
  (id: "shuffle_tail", bind: [], input: None,
   kind: Perform(ShuffleLibrary(who: Controller))),
  (id: "pause_after_tail", bind: [], input: None,
   kind: Choose(choice: "after_tail", chooser: Controller, optional: true,
    branches: [(branch_id: "continue", presentation: Fallback, cost: None,
     requirement: Always, body: (steps: [], yields: []))], otherwise: (steps: [], yields: [])))
 ], yields: []), target_bindings: [])))"#;

    #[test]
    fn direct_top_search_parks_program_context_and_draws_the_exact_forest_once() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;
        let (mut engine, top, owners, forest, wan) = nested_direct_top_fixture(
            DIRECT_TOP_CHOOSE_DRAW,
            "direct_top_choose_draw_fixture",
            91_251,
            true,
            false,
        );
        let forest = forest.expect("fixture includes one Forest");
        let forest_generation = engine.state.zone_change_generation[&forest];
        let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
        let player_one_hand_before = engine.state.players[1].hand.len();
        let wan_counters_before =
            engine.state.objects[&wan].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne);
        let drawn_slot = nested_slot_id(&engine, &owners[0], "drawn");
        let previous = nested_search_previous_result(&engine, wan);
        let seeded = previous.produced_objects[0];

        nested_start(&mut engine, &top, owners.clone(), previous);
        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        let (pending, stack, run) = parked_program(&engine);
        assert!(matches!(
            &pending.continuation,
            ResolutionContinuation::SearchLibrary { .. }
        ));
        assert_eq!(pending.deciding_player, 0);
        assert_eq!(pending.presentation.candidates, vec![forest]);
        assert_eq!(run.owner, owners[0]);
        assert_eq!(run.pending_owners, owners[1..]);
        assert_eq!(stack.previous_result.produced_objects, vec![seeded]);
        assert_eq!(run.in_flight, Some(Vec::new()));
        assert_eq!(run.slots.len(), 1);
        assert!(nested_slot(run, drawn_slot).is_none());
        assert_eq!(run.frames.len(), 2);
        assert!(run.frames.iter().flat_map(|frame| &frame.path).any(|path| {
            path.arm
                == ProgramArm::ChooseBranch(tricerules_card_model::ChoiceId::new("search").unwrap())
        }));

        let search_batch = nested_select_objects(&mut engine, vec![forest]);
        assert_eq!(
            nested_log_count(&search_batch, "P0 shuffles their library."),
            1
        );
        assert_eq!(nested_log_count(&search_batch, "P0 reveals Forest."), 1);
        let (pending, stack, run) = parked_program(&engine);
        assert!(matches!(
            &pending.continuation,
            ResolutionContinuation::ProgramChoose { .. }
        ));
        assert_eq!(run.owner, owners[0]);
        assert_eq!(run.pending_owners, owners[1..]);
        assert_eq!(stack.previous_result.produced_objects, vec![seeded]);
        assert!(matches!(
            nested_slot(run, drawn_slot),
            Some(ProgramValue::DrawReceipt(cards))
                if cards.len() == 1 && cards[0].object_id == forest
        ));
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            draws_before + 1
        );
        assert!(engine.state.players[0].hand.contains(&forest));
        assert_eq!(
            engine.state.zone_change_generation[&forest],
            forest_generation + 1
        );
        let resumed_choice = match &pending.continuation {
            ResolutionContinuation::ProgramChoose { choose, .. } => {
                choose.occurrence.choice_id.as_str()
            }
            other => panic!("expected resumed program Choose, got {other:?}"),
        };
        assert_eq!(resumed_choice, "after_draw");

        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            draws_before + 1
        );
        assert_eq!(
            engine.state.zone_change_generation[&forest],
            forest_generation + 1
        );
        assert!(engine.state.players[0].hand.contains(&forest));
        finish_one_wan_search_trigger(
            &mut engine,
            wan,
            player_one_hand_before,
            wan_counters_before,
        );
    }

    fn direct_top_search_zero_find_case(seed: u64, empty_library: bool) {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;
        let (mut engine, top, owners, _, wan) = nested_direct_top_fixture(
            DIRECT_TOP_CHOOSE_SAFE_TAIL,
            "direct_top_choose_safe_tail_fixture",
            seed,
            false,
            empty_library,
        );
        let library_before = engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
        let player_one_hand_before = engine.state.players[1].hand.len();
        let wan_counters_before =
            engine.state.objects[&wan].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne);
        let previous = nested_search_previous_result(&engine, wan);
        let seeded = previous.produced_objects[0];

        nested_start(&mut engine, &top, owners.clone(), previous);
        let branch_batch = nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        let search_batch = if empty_library {
            branch_batch
        } else {
            let (pending, stack, run) = parked_program(&engine);
            assert!(matches!(
                &pending.continuation,
                ResolutionContinuation::SearchLibrary { .. }
            ));
            // Keep the published maximum at authored capacity even when no candidate is eligible.
            assert_eq!((pending.presentation.min, pending.presentation.max), (0, 1));
            assert!(pending.presentation.candidates.is_empty());
            assert_eq!(stack.previous_result.produced_objects, vec![seeded]);
            assert_eq!(run.in_flight, Some(Vec::new()));
            nested_select_objects(&mut engine, Vec::new())
        };
        assert_eq!(nested_log_count(&search_batch, "P0 finds no card."), 1);
        assert_eq!(
            nested_log_count(&search_batch, "P0 shuffles their library."),
            2
        );
        assert_eq!(nested_log_count(&search_batch, "P0 reveals Forest."), 0);
        assert_eq!(
            engine.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            library_before
        );
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            draws_before
        );
        let (pending, stack, run) = parked_program(&engine);
        assert!(matches!(
            &pending.continuation,
            ResolutionContinuation::ProgramChoose { .. }
        ));
        assert_eq!(run.owner, owners[0]);
        assert_eq!(run.pending_owners, owners[1..]);
        assert_eq!(stack.previous_result.produced_objects, vec![seeded]);
        assert_eq!(run.in_flight, None);

        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            engine.state.turn_history.current.player(0).cards_drawn,
            draws_before
        );
        finish_one_wan_search_trigger(
            &mut engine,
            wan,
            player_one_hand_before,
            wan_counters_before,
        );
    }

    #[test]
    fn direct_top_search_nonempty_zero_find_resumes_into_safe_tail_once() {
        direct_top_search_zero_find_case(91_252, false);
    }

    #[test]
    fn direct_top_search_empty_library_shuffles_and_observes_before_safe_tail_once() {
        direct_top_search_zero_find_case(91_253, true);
    }

    #[test]
    fn declining_direct_top_search_only_runs_the_safe_tail_and_does_not_trigger_wan() {
        use tricerules_proto::ruled::v1::ResolutionChoiceDecision as Decision;
        let (mut engine, top, owners, _, wan) = nested_direct_top_fixture(
            DIRECT_TOP_CHOOSE_SAFE_TAIL,
            "direct_top_choose_safe_tail_fixture",
            91_254,
            false,
            false,
        );
        let library_before = engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let previous = nested_search_previous_result(&engine, wan);
        nested_start(&mut engine, &top, owners, previous);
        let decline_batch = nested_answer(&mut engine, Decision::Decline, 0, Vec::new());
        assert_eq!(
            nested_log_count(&decline_batch, "P0 shuffles their library."),
            1
        );
        assert_eq!(nested_log_count(&decline_batch, "P0 finds no card."), 0);
        assert_eq!(nested_log_count(&decline_batch, "P0 reveals Forest."), 0);
        assert_eq!(
            engine.state.players[0]
                .library
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            library_before
        );
        assert!(matches!(
            &engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .continuation,
            ResolutionContinuation::ProgramChoose { .. }
        ));
        nested_answer(&mut engine, Decision::SelectBranch, 0, Vec::new());
        assert!(engine.state.pending_resolution.is_none());
        assert!(
            engine.state.stack.is_empty(),
            "declining performs no library search trigger"
        );
    }

    #[test]
    fn stale_direct_top_search_selection_rejects_atomically() {
        let (mut engine, top, owners, forest, _) = nested_direct_top_fixture(
            DIRECT_TOP_CHOOSE_DRAW,
            "direct_top_choose_draw_fixture",
            91_255,
            true,
            false,
        );
        let forest = forest.expect("fixture includes one Forest");
        nested_start(
            &mut engine,
            &top,
            owners,
            crate::state::EffectResult::default(),
        );
        nested_answer(
            &mut engine,
            tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch,
            0,
            Vec::new(),
        );
        assert!(engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates
            .contains(&forest));
        move_object_to_zone(
            &mut engine.state,
            engine.registry,
            forest,
            Zone::Graveyard,
            None,
        )
        .expect("stale fixture Forest leaves the offered library incarnation");
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        let answer = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: pending.resolution_prompt_id.get(),
                    chosen_object_ids: vec![forest],
                    ..Default::default()
                },
            )),
        };
        let before = command_snapshot(&engine);
        assert!(matches!(
            engine.apply_scenario_command(0, &answer),
            Err(EngineError::Illegal(_))
        ));
        assert!(
            command_snapshot(&engine) == before,
            "stale result rejection preserves the exact offered continuation and candidate state"
        );
    }

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
