//! Finite, typed resolution programs for cards whose ordered actions need explicit results.
//!
//! Programs reuse the ordinary effect vocabulary for leaf actions. Their compiler gives results
//! stable slots and requires every branch to declare the values it yields to its parent.
//! The initial runner is deliberately narrower than that vocabulary: target groups bind only the
//! finite authored role signatures compiled below, while the remaining supported instructions
//! are controller-scoped spell/ability sequences. Draw and private hand-choice amounts are fixed
//! literals, `If` accepts only controller-library/hand-count conditions, and permanent choices
//! cannot use event-bound filters or cross-object constraints. Widening these rules needs a
//! separately typed context/result contract.

use super::{
    Amount, CardResultAction, CardTypeFilter, ChooseCardSource, EffectContext, EffectSubject,
    GameCondition, PlayerRecipient, RelativePlayerSet, SpellEffectKind, TargetFilter, TargetSchema,
    TargetingDef, TokenCopySource,
};
use crate::{ModeId, ProgramOwnerId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardProgram {
    pub body: AuthoredBlock,
    /// Stable instruction-to-group role references for this exact program owner.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_bindings: Vec<ProgramTargetReference>,
}

/// The finite target roles that authored resolution programs may bind to target groups.
///
/// These names are authoring coordinates; each accepted operation maps its one semantic role to
/// an existing `TargetRole` during compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProgramTargetRole {
    Target,
    Spell,
    Source,
    Subject,
}

/// Stable source reference from a target group to one authored program instruction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramTargetReference {
    pub instruction_id: String,
    pub role: ProgramTargetRole,
    pub target_group_index: u32,
}

/// Selects the target coordinates owned by a compiled program.
///
/// A mode-local projection uses its stable `ModeId` inside the exact parent carried by
/// `CompiledProgram::owner`; it never refers to the stack-wide flattened mode or target index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgramTargetProjection {
    OwnerLocal,
    ModeLocal { mode_id: ModeId },
}

/// Event-time values a compiled ability program requires from its captured trigger context.
///
/// This is a producer/consumer contract: registry admission proves that the trigger can supply
/// each value, and the engine checks the captured value before instruction zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ProgramRequiredInput {
    TriggerAffectedPlayer,
}

impl ProgramOwnerId {
    pub fn effect_context(&self) -> EffectContext {
        match self {
            Self::Face(_) | Self::FaceMode { .. } => EffectContext::Spell,
            Self::Ability(_) | Self::AbilityMode { .. } => EffectContext::Ability,
        }
    }

    pub fn target_projection(&self) -> ProgramTargetProjection {
        match self {
            Self::Face(_) | Self::Ability(_) => ProgramTargetProjection::OwnerLocal,
            Self::FaceMode { mode_id, .. } | Self::AbilityMode { mode_id, .. } => {
                ProgramTargetProjection::ModeLocal {
                    mode_id: mode_id.clone(),
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredBlock {
    #[serde(default)]
    pub steps: Vec<Instruction>,
    #[serde(default)]
    pub yields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instruction {
    pub id: String,
    #[serde(default)]
    pub bind: Vec<String>,
    #[serde(default)]
    pub input: Option<String>,
    pub kind: InstructionKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstructionKind {
    Perform(Box<EffectOp>),
    If {
        condition: Box<GameCondition>,
        yes: AuthoredBlock,
        no: AuthoredBlock,
    },
    IfResult {
        condition: ProgramResultCondition,
        yes: AuthoredBlock,
        no: AuthoredBlock,
    },
}

/// Internal finite condition form shared by authored and compiled instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramCondition {
    Game(GameCondition),
    Result(ProgramResultCondition),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgramResultCondition {
    DiscardCount {
        result: String,
        players: RelativePlayerSet,
        #[serde(default)]
        card_type: Option<CardTypeFilter>,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    DiscardOutcome {
        result: String,
        outcome: DiscardResultOutcome,
    },
}

/// The semantic outcome of one typed discard operation. Empty selection is a decline only for an
/// optional exact discard; lack of enough eligible cards is a separate unpayable result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscardResultOutcome {
    Performed,
    ChosenToPay,
    Declined,
    Unpayable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompiledProgramCondition {
    Game(GameCondition),
    Result(CompiledProgramResultCondition),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompiledProgramResultCondition {
    DiscardCount {
        result: ResultSlotId,
        players: RelativePlayerSet,
        card_type: Option<CardTypeFilter>,
        min: Option<u32>,
        max: Option<u32>,
    },
    DiscardOutcome {
        result: ResultSlotId,
        outcome: DiscardResultOutcome,
    },
}

/// Program leaves deliberately share the ordinary effect schema. The compiler below admits only
/// its finite supported subset, while registry validation rejects program-only leaves in lists.
pub type EffectOp = SpellEffectKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResultSlotId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cardinality {
    pub min: u32,
    pub max: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultType {
    DrawReceipt {
        who: PlayerRecipient,
        cardinality: Cardinality,
    },
    DiscardResult {
        who: PlayerRecipient,
        cardinality: Cardinality,
        optional_payment: bool,
    },
    PermanentSelection {
        chooser: PlayerRecipient,
        filter: Box<TargetFilter>,
        cardinality: Cardinality,
    },
    HandSelection {
        cardinality: Cardinality,
        ordered: bool,
    },
    CardResultCohort {
        action: CardResultAction,
        who: PlayerRecipient,
        cardinality: Cardinality,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledSlot {
    pub id: ResultSlotId,
    pub name: String,
    pub result_type: ResultType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledBlock {
    pub steps: Vec<CompiledInstruction>,
    pub yields: Vec<ResultSlotId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledInstruction {
    /// Stable pre-order address assigned once by the compiler across the entire conditional tree.
    #[serde(default)]
    pub instruction_address: u32,
    pub id: String,
    pub outputs: Vec<ResultSlotId>,
    pub input: Option<ResultSlotId>,
    pub kind: CompiledInstructionKind,
}

/// Owner-local target role binding compiled from the owner's validated TargetingDef.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramTargetBinding {
    pub instruction_address: u32,
    pub target_group_index: u32,
    pub role_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompiledInstructionKind {
    Perform(Box<EffectOp>),
    If {
        condition: Box<CompiledProgramCondition>,
        yes: CompiledBlock,
        no: CompiledBlock,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledProgram {
    pub owner: ProgramOwnerId,
    pub context: EffectContext,
    pub target_projection: ProgramTargetProjection,
    #[serde(default)]
    pub required_inputs: Vec<ProgramRequiredInput>,
    #[serde(default)]
    pub target_bindings: Vec<ProgramTargetBinding>,
    pub body: CompiledBlock,
    pub slots: Vec<CompiledSlot>,
}

struct Compiler {
    context: EffectContext,
    slots: Vec<CompiledSlot>,
    instruction_ids: HashSet<String>,
    required_inputs: std::collections::BTreeSet<ProgramRequiredInput>,
    next_instruction_address: u32,
}

impl CompiledProgram {
    pub fn compile(
        owner: ProgramOwnerId,
        context: EffectContext,
        program: &CardProgram,
    ) -> Result<Self, String> {
        Self::compile_with_targeting(owner, context, program, None)
    }

    /// Compile one authored program against its program-owned target-group declaration.
    pub fn compile_with_targeting(
        owner: ProgramOwnerId,
        context: EffectContext,
        program: &CardProgram,
        targeting: Option<&TargetingDef>,
    ) -> Result<Self, String> {
        let mut compiled = Self::compile_internal(owner, context, program)?;
        if context == EffectContext::Spell
            && targeting.is_some_and(TargetingDef::requires_opponent_choice)
        {
            return Err("opponent target choices require a supported activated ability".into());
        }
        if let Some(targeting) = targeting {
            for reference in &program.target_bindings {
                if reference.target_group_index as usize >= targeting.groups.len() {
                    return Err("program target binding references an unknown target group".into());
                }
                let (instruction_address, effect) =
                    find_instruction_by_id(&compiled.body, &reference.instruction_id).ok_or_else(
                        || {
                            format!(
                                "target binding references unknown program instruction '{}'",
                                reference.instruction_id
                            )
                        },
                    )?;
                let role_index =
                    authored_target_role_index(effect, reference.role).ok_or_else(|| {
                        format!(
                            "program instruction '{}' does not expose target role {:?}",
                            reference.instruction_id, reference.role
                        )
                    })?;
                compiled.target_bindings.push(ProgramTargetBinding {
                    instruction_address,
                    target_group_index: reference.target_group_index,
                    role_index: u32::try_from(role_index)
                        .map_err(|_| "target role index exceeds u32".to_string())?,
                });
            }
        } else if !program.target_bindings.is_empty() {
            return Err("program target binding requires an authored target group".into());
        }
        TargetSchema::compile_program(&compiled, targeting)?;
        Ok(compiled)
    }

    fn compile_internal(
        owner: ProgramOwnerId,
        context: EffectContext,
        program: &CardProgram,
    ) -> Result<Self, String> {
        if owner.effect_context() != context {
            return Err("program owner does not match the supplied effect context".into());
        }
        if program.body.steps.is_empty() {
            return Err("program body must contain at least one instruction".into());
        }
        let target_projection = owner.target_projection();
        let mut compiler = Compiler {
            context,
            slots: Vec::new(),
            instruction_ids: HashSet::new(),
            required_inputs: std::collections::BTreeSet::new(),
            next_instruction_address: 0,
        };
        let (body, _) = compiler.compile_block(&program.body, &HashMap::new())?;
        Ok(Self {
            owner,
            context,
            target_projection,
            required_inputs: compiler.required_inputs.into_iter().collect(),
            target_bindings: Vec::new(),
            body,
            slots: compiler.slots,
        })
    }

    pub fn slot(&self, id: ResultSlotId) -> Option<&CompiledSlot> {
        self.slots.get(id.0 as usize)
    }
}

fn authored_target_role_index(effect: &SpellEffectKind, role: ProgramTargetRole) -> Option<usize> {
    match (effect, role) {
        (SpellEffectKind::DamageTarget { .. }, ProgramTargetRole::Target)
        | (SpellEffectKind::CounterTargetSpell { .. }, ProgramTargetRole::Spell)
        | (
            SpellEffectKind::CreateTokenCopies {
                source: TokenCopySource::Chosen(_),
                ..
            },
            ProgramTargetRole::Source,
        )
        | (
            SpellEffectKind::ReturnToOwnersHand {
                subject: EffectSubject::Chosen(_),
            },
            ProgramTargetRole::Subject,
        ) => Some(0),
        _ => None,
    }
}

fn find_instruction_by_id<'a>(
    block: &'a CompiledBlock,
    id: &str,
) -> Option<(u32, &'a SpellEffectKind)> {
    for instruction in &block.steps {
        if instruction.id == id {
            if let CompiledInstructionKind::Perform(effect) = &instruction.kind {
                return Some((instruction.instruction_address, effect));
            }
            return None;
        }
        if let CompiledInstructionKind::If { yes, no, .. } = &instruction.kind {
            if let Some(found) = find_instruction_by_id(yes, id) {
                return Some(found);
            }
            if let Some(found) = find_instruction_by_id(no, id) {
                return Some(found);
            }
        }
    }
    None
}

impl Compiler {
    fn compile_block(
        &mut self,
        authored: &AuthoredBlock,
        inherited: &HashMap<String, ResultSlotId>,
    ) -> Result<(CompiledBlock, Vec<ResultType>), String> {
        let mut names = inherited.clone();
        let mut steps = Vec::with_capacity(authored.steps.len());
        for instruction in &authored.steps {
            if instruction.id.trim().is_empty()
                || !self.instruction_ids.insert(instruction.id.clone())
            {
                return Err(format!(
                    "duplicate or empty instruction id '{}'",
                    instruction.id
                ));
            }
            let instruction_address = self.next_instruction_address;
            self.next_instruction_address = self
                .next_instruction_address
                .checked_add(1)
                .ok_or_else(|| "program instruction address exceeds u32".to_string())?;
            let (input, result_types, kind) = match &instruction.kind {
                InstructionKind::Perform(effect) => {
                    let input =
                        compile_input(effect, instruction.input.as_deref(), &names, &self.slots)?;
                    if operation_needs_affected_player(effect) {
                        self.required_inputs
                            .insert(ProgramRequiredInput::TriggerAffectedPlayer);
                    }
                    let result_types = validate_and_result_types(effect, self.context)?;
                    (
                        input,
                        result_types,
                        CompiledInstructionKind::Perform(effect.clone()),
                    )
                }
                InstructionKind::If { condition, yes, no } => {
                    if instruction.input.is_some() {
                        return Err(format!(
                            "If instruction '{}' cannot consume an input result",
                            instruction.id
                        ));
                    }
                    let condition = compile_program_condition(
                        &ProgramCondition::Game(condition.as_ref().clone()),
                        &names,
                        &self.slots,
                    )
                    .map_err(|reason| format!("If instruction '{}': {reason}", instruction.id))?;
                    let (yes, yes_types) = self.compile_block(yes, &names)?;
                    let (no, no_types) = self.compile_block(no, &names)?;
                    if yes_types.len() != no_types.len() {
                        return Err(format!(
                            "If instruction '{}' has mismatched explicit yield arity",
                            instruction.id
                        ));
                    }
                    let mut joined = Vec::with_capacity(yes_types.len());
                    for (yes_type, no_type) in yes_types.into_iter().zip(no_types) {
                        joined.push(join_result_types(&yes_type, &no_type).ok_or_else(|| {
                            format!(
                                "If instruction '{}' has mismatched explicit yield types",
                                instruction.id
                            )
                        })?);
                    }
                    (
                        None,
                        joined,
                        CompiledInstructionKind::If {
                            condition: Box::new(condition),
                            yes,
                            no,
                        },
                    )
                }
                InstructionKind::IfResult { condition, yes, no } => {
                    if instruction.input.is_some() {
                        return Err(format!(
                            "IfResult instruction '{}' cannot consume an input result",
                            instruction.id
                        ));
                    }
                    let condition = compile_program_condition(
                        &ProgramCondition::Result(condition.clone()),
                        &names,
                        &self.slots,
                    )
                    .map_err(|reason| {
                        format!("IfResult instruction '{}': {reason}", instruction.id)
                    })?;
                    let (yes, yes_types) = self.compile_block(yes, &names)?;
                    let (no, no_types) = self.compile_block(no, &names)?;
                    if yes_types.len() != no_types.len() {
                        return Err(format!(
                            "IfResult instruction '{}' has mismatched explicit yield arity",
                            instruction.id
                        ));
                    }
                    let mut joined = Vec::with_capacity(yes_types.len());
                    for (yes_type, no_type) in yes_types.into_iter().zip(no_types) {
                        joined.push(join_result_types(&yes_type, &no_type).ok_or_else(|| {
                            format!(
                                "IfResult instruction '{}' has mismatched explicit yield types",
                                instruction.id
                            )
                        })?);
                    }
                    (
                        None,
                        joined,
                        CompiledInstructionKind::If {
                            condition: Box::new(condition),
                            yes,
                            no,
                        },
                    )
                }
            };
            let outputs = bind_outputs(
                &instruction.id,
                &instruction.bind,
                &result_types,
                &mut names,
                &mut self.slots,
            )?;
            steps.push(CompiledInstruction {
                instruction_address,
                id: instruction.id.clone(),
                outputs,
                input,
                kind,
            });
        }
        let mut yields = Vec::with_capacity(authored.yields.len());
        let mut result_types = Vec::with_capacity(authored.yields.len());
        for name in &authored.yields {
            let slot = names
                .get(name)
                .copied()
                .ok_or_else(|| format!("block yields result '{name}' outside its scope"))?;
            let result_type = self
                .slots
                .get(slot.0 as usize)
                .ok_or_else(|| format!("block yields unknown result '{name}'"))?
                .result_type
                .clone();
            yields.push(slot);
            result_types.push(result_type);
        }
        Ok((CompiledBlock { steps, yields }, result_types))
    }
}

fn operation_needs_affected_player(effect: &SpellEffectKind) -> bool {
    matches!(
        effect,
        SpellEffectKind::Draw {
            who: PlayerRecipient::AffectedPlayer,
            ..
        } | SpellEffectKind::Discard {
            who: PlayerRecipient::AffectedPlayer,
            ..
        } | SpellEffectKind::Mill {
            who: PlayerRecipient::AffectedPlayer,
            ..
        }
    )
}

fn compile_program_condition(
    condition: &ProgramCondition,
    names: &HashMap<String, ResultSlotId>,
    slots: &[CompiledSlot],
) -> Result<CompiledProgramCondition, String> {
    match condition {
        ProgramCondition::Game(condition) => {
            validate_program_condition(condition)?;
            Ok(CompiledProgramCondition::Game(condition.clone()))
        }
        ProgramCondition::Result(ProgramResultCondition::DiscardCount {
            result,
            players,
            card_type,
            min,
            max,
        }) => {
            if min.is_none() && max.is_none() {
                return Err("discard result count requires a minimum or maximum".into());
            }
            if min
                .zip(*max)
                .is_some_and(|(minimum, maximum)| minimum > maximum)
            {
                return Err("discard result count minimum cannot exceed its maximum".into());
            }
            let slot = result_slot(result, names, slots)?;
            let ResultType::DiscardResult {
                who, cardinality, ..
            } = &slots[slot.0 as usize].result_type
            else {
                return Err(format!(
                    "result '{result}' is not a compatible discard result"
                ));
            };
            if *who != PlayerRecipient::Controller || *players != RelativePlayerSet::Controller {
                return Err(format!(
                    "discard result '{result}' does not expose the requested player cohort"
                ));
            }
            if cardinality.max.is_some_and(|upper| {
                min.is_some_and(|minimum| minimum > upper) || max.is_some_and(|bound| bound > upper)
            }) {
                return Err(format!(
                    "discard result '{result}' predicate exceeds its typed cohort cardinality"
                ));
            }
            Ok(CompiledProgramCondition::Result(
                CompiledProgramResultCondition::DiscardCount {
                    result: slot,
                    players: *players,
                    card_type: *card_type,
                    min: *min,
                    max: *max,
                },
            ))
        }
        ProgramCondition::Result(ProgramResultCondition::DiscardOutcome { result, outcome }) => {
            let slot = result_slot(result, names, slots)?;
            let ResultType::DiscardResult {
                optional_payment, ..
            } = &slots[slot.0 as usize].result_type
            else {
                return Err(format!(
                    "result '{result}' is not a compatible discard result"
                ));
            };
            if !optional_payment && *outcome != DiscardResultOutcome::Performed {
                return Err(format!(
                    "result '{result}' does not represent an optional discard payment"
                ));
            }
            Ok(CompiledProgramCondition::Result(
                CompiledProgramResultCondition::DiscardOutcome {
                    result: slot,
                    outcome: *outcome,
                },
            ))
        }
    }
}

fn result_slot(
    name: &str,
    names: &HashMap<String, ResultSlotId>,
    slots: &[CompiledSlot],
) -> Result<ResultSlotId, String> {
    let slot = names
        .get(name)
        .copied()
        .ok_or_else(|| format!("result '{name}' is not in scope"))?;
    if slots.get(slot.0 as usize).is_none() {
        return Err(format!("result '{name}' is unknown"));
    }
    Ok(slot)
}

fn compile_input(
    effect: &EffectOp,
    input: Option<&str>,
    names: &HashMap<String, ResultSlotId>,
    slots: &[CompiledSlot],
) -> Result<Option<ResultSlotId>, String> {
    let expected = match effect {
        EffectOp::UntapChosenPermanents => Some("PermanentSelection"),
        EffectOp::PutSelectedHandCardsOnLibraryTop => Some("HandSelection"),
        EffectOp::ChooseCard {
            source: ChooseCardSource::CardResultCohort { .. },
            ..
        } => Some("CardResultCohort"),
        _ => None,
    };
    match (expected, input) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err("instruction does not accept an input result".into()),
        (Some(_), None) => Err("instruction requires an explicitly bound input result".into()),
        (Some(expected), Some(name)) => {
            let slot = names
                .get(name)
                .copied()
                .ok_or_else(|| format!("input result '{name}' is not in scope"))?;
            let actual = slots
                .get(slot.0 as usize)
                .ok_or_else(|| format!("input result '{name}' is unknown"))?;
            let matches = match effect {
                EffectOp::ChooseCard {
                    source: ChooseCardSource::CardResultCohort { .. },
                    ..
                } => matches!(
                    actual.result_type,
                    ResultType::CardResultCohort {
                        action: CardResultAction::Mill,
                        ..
                    }
                ),
                _ => matches!(
                    (&actual.result_type, expected),
                    (ResultType::PermanentSelection { .. }, "PermanentSelection")
                        | (
                            ResultType::HandSelection { ordered: true, .. },
                            "HandSelection"
                        )
                ),
            };
            if !matches {
                return Err(format!(
                    "input result '{name}' has the wrong type for this instruction"
                ));
            }
            Ok(Some(slot))
        }
    }
}

fn bind_outputs(
    instruction_id: &str,
    names_to_bind: &[String],
    result_types: &[ResultType],
    names: &mut HashMap<String, ResultSlotId>,
    slots: &mut Vec<CompiledSlot>,
) -> Result<Vec<ResultSlotId>, String> {
    if names_to_bind.is_empty() {
        return Ok(Vec::new());
    }
    if names_to_bind.len() != result_types.len() {
        return Err(format!(
            "instruction '{instruction_id}' binds {} names for {} results",
            names_to_bind.len(),
            result_types.len()
        ));
    }
    let mut outputs = Vec::with_capacity(names_to_bind.len());
    for (name, result_type) in names_to_bind.iter().zip(result_types) {
        if name.trim().is_empty() || names.contains_key(name) {
            return Err(format!(
                "instruction '{instruction_id}' binds empty or already scoped result '{name}'"
            ));
        }
        let id = ResultSlotId(
            u32::try_from(slots.len()).map_err(|_| "program has too many result slots")?,
        );
        slots.push(CompiledSlot {
            id,
            name: name.clone(),
            result_type: result_type.clone(),
        });
        names.insert(name.clone(), id);
        outputs.push(id);
    }
    Ok(outputs)
}

fn join_result_types(left: &ResultType, right: &ResultType) -> Option<ResultType> {
    fn union(left: Cardinality, right: Cardinality) -> Cardinality {
        Cardinality {
            min: left.min.min(right.min),
            max: match (left.max, right.max) {
                (Some(a), Some(b)) => Some(a.max(b)),
                _ => None,
            },
        }
    }
    Some(match (left, right) {
        (
            ResultType::DrawReceipt {
                who: a,
                cardinality: ac,
            },
            ResultType::DrawReceipt {
                who: b,
                cardinality: bc,
            },
        ) if a == b => ResultType::DrawReceipt {
            who: *a,
            cardinality: union(*ac, *bc),
        },
        (
            ResultType::DiscardResult {
                who: a,
                cardinality: ac,
                optional_payment: ao,
            },
            ResultType::DiscardResult {
                who: b,
                cardinality: bc,
                optional_payment: bo,
            },
        ) if a == b && ao == bo => ResultType::DiscardResult {
            who: *a,
            cardinality: union(*ac, *bc),
            optional_payment: *ao,
        },
        (
            ResultType::PermanentSelection {
                chooser: a,
                filter: af,
                cardinality: ac,
            },
            ResultType::PermanentSelection {
                chooser: b,
                filter: bf,
                cardinality: bc,
            },
        ) if a == b && af == bf => ResultType::PermanentSelection {
            chooser: *a,
            filter: af.clone(),
            cardinality: union(*ac, *bc),
        },
        (
            ResultType::HandSelection {
                cardinality: ac,
                ordered: ao,
            },
            ResultType::HandSelection {
                cardinality: bc,
                ordered: bo,
            },
        ) if ao == bo => ResultType::HandSelection {
            cardinality: union(*ac, *bc),
            ordered: *ao,
        },
        (
            ResultType::CardResultCohort {
                action: aa,
                who: aw,
                cardinality: ac,
            },
            ResultType::CardResultCohort {
                action: ba,
                who: bw,
                cardinality: bc,
            },
        ) if aa == ba && aw == bw => ResultType::CardResultCohort {
            action: *aa,
            who: *aw,
            cardinality: union(*ac, *bc),
        },
        _ => return None,
    })
}

fn validate_and_result_types(
    effect: &SpellEffectKind,
    context: EffectContext,
) -> Result<Vec<ResultType>, String> {
    match effect {
        SpellEffectKind::Draw { who, count } => {
            effect.validate(context)?;
            validate_program_recipient(*who, context, "Draw recipient")?;
            validate_program_amount(count)?;
            validate_fixed_program_amount("Draw count", count)?;
            // Replacement effects may expand the committed draw cohort, so the authored count
            // does not bound the receipt.
            Ok(vec![ResultType::DrawReceipt {
                who: *who,
                cardinality: Cardinality { min: 0, max: None },
            }])
        }
        SpellEffectKind::Discard { who, quantity } => {
            effect.validate(context)?;
            validate_program_recipient(*who, context, "Discard recipient")?;
            let optional_payment = matches!(quantity, super::DiscardQuantity::OptionalExact(_));
            Ok(vec![ResultType::DiscardResult {
                who: *who,
                cardinality: Cardinality {
                    min: 0,
                    max: discard_upper_bound(quantity),
                },
                optional_payment,
            }])
        }
        SpellEffectKind::ChoosePermanents {
            chooser,
            filter,
            max,
            constraints,
            ..
        } => {
            effect.validate(context)?;
            validate_program_controller(*chooser, "ChoosePermanents chooser")?;
            validate_program_choice_filter(filter)?;
            if !constraints.is_empty() {
                return Err("ChoosePermanents constraints are not supported in spell_program without typed object references".into());
            }
            // Selection can be limited by the live candidate set, even when the authored minimum
            // is larger.
            Ok(vec![ResultType::PermanentSelection {
                chooser: *chooser,
                filter: Box::new(filter.clone()),
                cardinality: Cardinality {
                    min: 0,
                    max: Some(*max),
                },
            }])
        }
        SpellEffectKind::UntapChosenPermanents => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::SelectHandCards {
            count,
            ordered,
            prompt,
        } => {
            count.validate_effect(context)?;
            validate_program_amount(count)?;
            validate_fixed_program_amount("SelectHandCards count", count)?;
            if prompt.trim().is_empty() {
                return Err("SelectHandCards requires a nonempty prompt".into());
            }
            Ok(vec![ResultType::HandSelection {
                cardinality: Cardinality {
                    min: 0,
                    max: amount_upper_bound(count),
                },
                ordered: *ordered,
            }])
        }
        SpellEffectKind::Mill { count, who } => {
            effect.validate(context)?;
            validate_program_recipient(*who, context, "Mill recipient")?;
            validate_program_amount(count)?;
            Ok(vec![ResultType::CardResultCohort {
                action: CardResultAction::Mill,
                who: *who,
                cardinality: Cardinality {
                    min: 0,
                    max: amount_upper_bound(count),
                },
            }])
        }
        SpellEffectKind::ChooseCard {
            filter,
            destination,
            ..
        } => {
            filter.validate()?;
            if !matches!(
                destination,
                super::GraveyardDestination::Hand
                    | super::GraveyardDestination::LibraryTop
                    | super::GraveyardDestination::LibraryBottom
            ) {
                return Err(
                    "ChooseCard currently supports only Hand and Library destinations in spell_program"
                        .into(),
                );
            }
            Ok(Vec::new())
        }
        SpellEffectKind::PutSelectedHandCardsOnLibraryTop => Ok(Vec::new()),
        SpellEffectKind::DamageTarget { amount, target } => {
            effect.validate(context)?;
            validate_program_amount(amount)?;
            match amount {
                Amount::Fixed(_) => {}
                Amount::X if context == EffectContext::Spell => {}
                _ => {
                    return Err(
                        "DamageTarget amount must be fixed, or X on a resolving spell".into(),
                    )
                }
            }
            validate_program_target_filter(target, "DamageTarget target")?;
            Ok(Vec::new())
        }
        SpellEffectKind::CounterTargetSpell {
            unless_controller_pays,
            unless_controller_pays_by_cast_cost,
            ..
        } => {
            effect.validate(context)?;
            if unless_controller_pays_by_cast_cost.is_some() {
                return Err(
                    "CounterTargetSpell cast-cost payment is outside the typed program input contract"
                        .into(),
                );
            }
            if let Some(amount) = unless_controller_pays {
                validate_program_amount(amount)?;
                validate_fixed_program_amount("CounterTargetSpell payment", amount)?;
            }
            Ok(Vec::new())
        }
        SpellEffectKind::ReturnToOwnersHand { subject } => {
            effect.validate(context)?;
            let super::EffectSubject::Chosen(filter) = subject else {
                return Err(
                    "ReturnToOwnersHand requires an owner-local chosen target in a resolution program"
                        .into(),
                );
            };
            validate_program_target_filter(filter, "ReturnToOwnersHand target")?;
            Ok(Vec::new())
        }
        SpellEffectKind::CreateTokens {
            count,
            who,
            sacrifice_timing,
            ..
        } => {
            effect.validate(context)?;
            if *who != PlayerRecipient::Controller || sacrifice_timing.is_some() {
                return Err(
                    "CreateTokens supports controller-owned ordinary tokens without delayed sacrifice"
                        .into(),
                );
            }
            validate_program_amount(count)?;
            validate_fixed_program_amount("CreateTokens count", count)?;
            Ok(Vec::new())
        }
        SpellEffectKind::CreateTokenCopies { count, source } => {
            effect.validate(context)?;
            let super::TokenCopySource::Chosen(filter) = source else {
                return Err(
                    "CreateTokenCopies requires an owner-local chosen target in a resolution program"
                        .into(),
                );
            };
            validate_program_amount(count)?;
            validate_fixed_program_amount("CreateTokenCopies count", count)?;
            validate_program_target_filter(filter, "CreateTokenCopies target")?;
            Ok(Vec::new())
        }
        _ => Err("effect is not in the resolution-program operation whitelist".into()),
    }
}

fn validate_program_controller(recipient: PlayerRecipient, role: &str) -> Result<(), String> {
    if recipient == PlayerRecipient::Controller {
        Ok(())
    } else {
        Err(format!(
            "{role} supports only the resolving spell controller in spell_program"
        ))
    }
}

fn validate_program_recipient(
    recipient: PlayerRecipient,
    context: EffectContext,
    role: &str,
) -> Result<(), String> {
    match recipient {
        PlayerRecipient::Controller => Ok(()),
        PlayerRecipient::AffectedPlayer if context == EffectContext::Ability => Ok(()),
        _ if context == EffectContext::Spell => Err(format!(
            "{role} supports only the resolving spell controller in spell_program"
        )),
        _ => Err(format!(
            "{role} uses a recipient outside the finite resolution-program input contract"
        )),
    }
}

fn validate_program_condition(condition: &GameCondition) -> Result<(), String> {
    match condition {
        GameCondition::ControllerLibraryEmpty => {}
        GameCondition::CardsInHand {
            players: super::ConditionPlayerSet::Relative(super::RelativePlayerSet::Controller),
            ..
        } => {}
        GameCondition::AllOf(branches) | GameCondition::AnyOf(branches) => {
            for branch in branches {
                validate_program_condition(branch)?;
            }
        }
        _ => {
            return Err(
                "If condition uses context outside controller hand and library state supported by spell_program"
                    .into(),
            )
        }
    }
    condition.validate_live()
}

fn validate_program_choice_filter(filter: &TargetFilter) -> Result<(), String> {
    if filter.controller == super::TargetController::DefendingPlayer {
        return Err("ChoosePermanents filter cannot use DefendingPlayer in spell_program".into());
    }
    if filter
        .excluded_objects
        .contains(&super::TargetObjectExclusion::AttachedObject)
    {
        return Err("ChoosePermanents filter cannot use AttachedObject in spell_program".into());
    }
    if let Some(branches) = &filter.any_of {
        for branch in branches {
            validate_program_choice_filter(branch)?;
        }
    }
    filter.validate_characteristic_constraints()
}

fn validate_program_target_filter(filter: &TargetFilter, role: &str) -> Result<(), String> {
    if filter.controller == super::TargetController::DefendingPlayer {
        return Err(format!(
            "{role} cannot depend on a defending player outside its typed program inputs"
        ));
    }
    if filter
        .excluded_objects
        .contains(&super::TargetObjectExclusion::AttachedObject)
    {
        return Err(format!(
            "{role} cannot depend on an attached object outside its typed program inputs"
        ));
    }
    if let Some(branches) = &filter.any_of {
        for branch in branches {
            validate_program_target_filter(branch, role)?;
        }
    }
    filter.validate_target_constraints()
}

fn validate_fixed_program_amount(role: &str, amount: &Amount) -> Result<(), String> {
    if matches!(amount, Amount::Fixed(_)) {
        Ok(())
    } else {
        Err(format!(
            "{role} supports only a fixed literal in spell_program"
        ))
    }
}

fn validate_program_amount(amount: &Amount) -> Result<(), String> {
    amount.validate_live()?;
    if amount.card_result_filter().is_some() {
        return Err("program amounts cannot read an implicit card result; result consumers require typed bindings".into());
    }
    match amount {
        Amount::Conditional { condition, .. } => condition.validate_live(),
        Amount::DivideRoundedDown { amount, .. } => validate_program_amount(amount),
        _ => Ok(()),
    }
}

fn amount_upper_bound(amount: &Amount) -> Option<u32> {
    match amount {
        Amount::Fixed(value) => Some(*value),
        Amount::Conditional {
            when_true,
            otherwise,
            ..
        } => Some((*when_true).max(*otherwise)),
        Amount::DivideRoundedDown { amount, .. } => amount_upper_bound(amount),
        Amount::X | Amount::CastCost(_) | Amount::Count(_) | Amount::EventCount => None,
    }
}

fn discard_upper_bound(quantity: &super::DiscardQuantity) -> Option<u32> {
    match quantity {
        super::DiscardQuantity::Exact(count)
        | super::DiscardQuantity::OptionalExact(count)
        | super::DiscardQuantity::UpTo(count) => Some(*count),
        super::DiscardQuantity::AnyNumber | super::DiscardQuantity::All => None,
        super::DiscardQuantity::UnlessOne { count, .. } => Some(*count),
    }
}
