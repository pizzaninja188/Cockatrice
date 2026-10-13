//! Finite, typed resolution programs for cards whose ordered actions need explicit results.
//!
//! Programs reuse the ordinary effect vocabulary for leaf actions. Their compiler gives results
//! stable slots and requires every branch to declare the values it yields to its parent.
//! The initial runner is deliberately narrower than that vocabulary: spell programs are
//! targetless, controller-scoped instant/sorcery sequences. Draw and private hand-choice amounts
//! are fixed literals, `If` accepts only controller-library/hand-count conditions, and permanent
//! choices cannot use event-bound filters or cross-object constraints. Widening these rules needs
//! a separately typed context/result contract.

use super::{
    Amount, CardTypeFilter, DiscardQuantity, DrawDiscardOrder, EffectContext, GameCondition,
    PlayerRecipient, RelativePlayerSet, ResolutionBranchRequirement, SpellEffectKind, TargetFilter,
    TargetSchema, TargetingDef,
};
use crate::{ModeId, ProgramOwnerId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardProgram {
    pub body: AuthoredBlock,
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
    /// Source-only provenance attached by legacy lowering. It is omitted from authored RON;
    /// compiled instructions retain it for owner-local target binding.
    #[serde(skip)]
    source_effect_path: Option<SourceEffectPath>,
}

/// Structural address of the effect in the source owner that produced a compiled instruction.
/// The enclosing `CompiledProgram.owner` scopes the root index to a face, ability, or stable mode.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceEffectPath {
    pub root_effect_index: u32,
    #[serde(default)]
    pub nested_steps: Vec<SourceEffectPathStep>,
}

/// Edges through source wrappers and nested effect lists. List indices remain local to their
/// immediate parent; the root source effect ordinal is never inferred from expanded instruction
/// positions or generated IDs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceEffectPathStep {
    ConditionalEffect,
    ConditionalCastCostEffect,
    BranchEffect {
        branch_id: crate::ChoiceId,
        effect_index: u32,
    },
    OtherwiseEffect {
        effect_index: u32,
    },
    IfBeheldEffect {
        effect_index: u32,
    },
}

impl SourceEffectPath {
    fn root(root_effect_index: u32) -> Self {
        Self {
            root_effect_index,
            nested_steps: Vec::new(),
        }
    }
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
    pub id: String,
    pub outputs: Vec<ResultSlotId>,
    pub input: Option<ResultSlotId>,
    #[serde(default)]
    pub source_effect_path: Option<SourceEffectPath>,
    pub kind: CompiledInstructionKind,
}

/// Owner-local target role binding compiled from the owner's validated TargetingDef.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramTargetBinding {
    pub source_effect_path: SourceEffectPath,
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
    preserve_source_paths: bool,
}

impl CompiledProgram {
    pub fn compile(
        owner: ProgramOwnerId,
        context: EffectContext,
        program: &CardProgram,
    ) -> Result<Self, String> {
        Self::compile_with_policy(owner, context, program, false)
    }

    /// Normalize and compile one complete legacy effect body. The registry calls this only when
    /// the exact owner contains DrawDiscard; its siblings are retained in their original order.
    pub(crate) fn compile_legacy_effects(
        owner: ProgramOwnerId,
        effects: &[SpellEffectKind],
        targeting: Option<&TargetingDef>,
    ) -> Result<Self, String> {
        let schema = TargetSchema::compile(effects, targeting)?;
        if schema
            .groups
            .iter()
            .flat_map(|group| &group.bindings)
            .any(|binding| {
                matches!(
                    effects.get(binding.effect_index),
                    Some(
                        SpellEffectKind::Conditional { .. }
                            | SpellEffectKind::ConditionalCastCost { .. }
                    )
                )
            })
        {
            return Err(
                "nested target roles inside Conditional or ConditionalCastCost are not yet supported in resolution programs".into(),
            );
        }
        let body = lower_legacy_effects(effects, "source")?;
        let program = CardProgram { body };
        let mut compiled =
            Self::compile_with_policy(owner.clone(), owner.effect_context(), &program, true)?;
        for (group_index, group) in schema.groups.iter().enumerate() {
            for binding in &group.bindings {
                compiled.target_bindings.push(ProgramTargetBinding {
                    source_effect_path: SourceEffectPath::root(
                        u32::try_from(binding.effect_index)
                            .map_err(|_| "targeted source effect index exceeds u32".to_string())?,
                    ),
                    target_group_index: u32::try_from(group_index)
                        .map_err(|_| "target group index exceeds u32".to_string())?,
                    role_index: u32::try_from(binding.role_index)
                        .map_err(|_| "target role index exceeds u32".to_string())?,
                });
            }
        }
        Ok(compiled)
    }

    fn compile_with_policy(
        owner: ProgramOwnerId,
        context: EffectContext,
        program: &CardProgram,
        preserve_source_paths: bool,
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
            preserve_source_paths,
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
                id: instruction.id.clone(),
                outputs,
                input,
                source_effect_path: if self.preserve_source_paths {
                    instruction.source_effect_path.clone()
                } else {
                    None
                },
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

fn lower_legacy_effects(
    effects: &[SpellEffectKind],
    prefix: &str,
) -> Result<AuthoredBlock, String> {
    let source_paths = effects
        .iter()
        .enumerate()
        .map(|(index, _)| {
            Ok(SourceEffectPath {
                root_effect_index: u32::try_from(index)
                    .map_err(|_| "source effect list is too large")?,
                nested_steps: Vec::new(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    lower_legacy_effects_with_paths(effects, prefix, &source_paths)
}

fn lower_legacy_branch_effects(
    effects: &[SpellEffectKind],
    prefix: &str,
    parent: &SourceEffectPath,
    branch_id: &crate::ChoiceId,
) -> Result<AuthoredBlock, String> {
    let source_paths = effects
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let mut path = parent.clone();
            path.nested_steps.push(SourceEffectPathStep::BranchEffect {
                branch_id: branch_id.clone(),
                effect_index: u32::try_from(index)
                    .map_err(|_| "source branch effect list is too large")?,
            });
            Ok(path)
        })
        .collect::<Result<Vec<_>, String>>()?;
    lower_legacy_effects_with_paths(effects, prefix, &source_paths)
}

fn lower_legacy_effects_with_paths(
    effects: &[SpellEffectKind],
    prefix: &str,
    source_paths: &[SourceEffectPath],
) -> Result<AuthoredBlock, String> {
    if effects.len() != source_paths.len() {
        return Err("source effect path count does not match the effect list".into());
    }
    let mut steps = Vec::new();
    let mut previous_discard_result = None;
    let mut previous_selection_result = None;

    for (index, (effect, source_effect_path)) in effects.iter().zip(source_paths).enumerate() {
        let path = format!("{prefix}_{index:03}");
        match effect {
            SpellEffectKind::DrawDiscard {
                who,
                draw_count,
                discard_count,
                order,
                optional,
            } => {
                if *optional && (*order != DrawDiscardOrder::DiscardThenDraw || *discard_count != 1)
                {
                    return Err(format!(
                        "{path}: optional DrawDiscard requires DiscardThenDraw with one discarded card"
                    ));
                }
                let draw = SpellEffectKind::Draw {
                    who: *who,
                    count: Amount::Fixed(*draw_count),
                };
                let discard = SpellEffectKind::Discard {
                    who: *who,
                    quantity: if *optional {
                        DiscardQuantity::OptionalExact(*discard_count)
                    } else {
                        DiscardQuantity::Exact(*discard_count)
                    },
                };
                let discard_result = format!("{path}_discard_result");
                if *optional {
                    steps.push(Instruction {
                        id: format!("{path}_discard"),
                        bind: vec![discard_result.clone()],
                        input: None,
                        kind: InstructionKind::Perform(Box::new(discard)),
                        source_effect_path: Some(source_effect_path.clone()),
                    });
                    steps.push(Instruction {
                        id: format!("{path}_draw_if_paid"),
                        bind: Vec::new(),
                        input: None,
                        kind: InstructionKind::IfResult {
                            condition: ProgramResultCondition::DiscardOutcome {
                                result: discard_result.clone(),
                                outcome: DiscardResultOutcome::ChosenToPay,
                            },
                            yes: AuthoredBlock {
                                steps: vec![source_perform(
                                    format!("{path}_paid_000"),
                                    draw,
                                    None,
                                    None,
                                    source_effect_path.clone(),
                                )],
                                yields: Vec::new(),
                            },
                            no: AuthoredBlock {
                                steps: Vec::new(),
                                yields: Vec::new(),
                            },
                        },
                        source_effect_path: Some(source_effect_path.clone()),
                    });
                } else {
                    let (first, second) = match order {
                        DrawDiscardOrder::DrawThenDiscard => (&draw, &discard),
                        DrawDiscardOrder::DiscardThenDraw => (&discard, &draw),
                    };
                    steps.push(source_perform(
                        format!("{path}_first"),
                        first.clone(),
                        (std::ptr::eq(first, &discard)).then(|| discard_result.clone()),
                        None,
                        source_effect_path.clone(),
                    ));
                    steps.push(source_perform(
                        format!("{path}_second"),
                        second.clone(),
                        (std::ptr::eq(second, &discard)).then(|| discard_result.clone()),
                        None,
                        source_effect_path.clone(),
                    ));
                }
                previous_discard_result = Some(discard_result);
                previous_selection_result = None;
            }
            SpellEffectKind::ChooseResolutionBranch { .. } if previous_discard_result.is_some() => {
                let Some((condition, branch_id, yes_effects, no_effects)) =
                    typed_discard_branch(effect, previous_discard_result.as_deref().unwrap())
                else {
                    return Err(format!(
                        "{path}: a PreviousEffect discard condition must be lowered to an explicit typed result branch"
                    ));
                };
                steps.push(Instruction {
                    id: format!("{path}_result_branch"),
                    bind: Vec::new(),
                    input: None,
                    kind: InstructionKind::IfResult {
                        condition,
                        yes: lower_legacy_branch_effects(
                            &yes_effects,
                            &format!("{path}_yes"),
                            source_effect_path,
                            &branch_id,
                        )?,
                        no: lower_legacy_effects(&no_effects, &format!("{path}_no"))?,
                    },
                    source_effect_path: Some(source_effect_path.clone()),
                });
                previous_discard_result = None;
                previous_selection_result = None;
            }
            _ => {
                let input = match effect {
                    SpellEffectKind::UntapChosenPermanents => previous_selection_result.take(),
                    SpellEffectKind::PutSelectedHandCardsOnLibraryTop => {
                        previous_selection_result.take()
                    }
                    _ => None,
                };
                let bind = match effect {
                    SpellEffectKind::Discard { .. } => {
                        let result = format!("{path}_discard_result");
                        previous_discard_result = Some(result.clone());
                        vec![result]
                    }
                    SpellEffectKind::ChoosePermanents { .. }
                    | SpellEffectKind::SelectHandCards { .. } => {
                        let result = format!("{path}_selection_result");
                        previous_selection_result = Some(result.clone());
                        vec![result]
                    }
                    _ => {
                        previous_discard_result = None;
                        previous_selection_result = None;
                        Vec::new()
                    }
                };
                steps.push(Instruction {
                    id: path,
                    bind,
                    input,
                    kind: InstructionKind::Perform(Box::new(effect.clone())),
                    source_effect_path: Some(source_effect_path.clone()),
                });
            }
        }
    }

    Ok(AuthoredBlock {
        steps,
        yields: Vec::new(),
    })
}

fn source_perform(
    id: String,
    effect: SpellEffectKind,
    result: Option<String>,
    input: Option<String>,
    source_effect_path: SourceEffectPath,
) -> Instruction {
    Instruction {
        id,
        bind: result.into_iter().collect(),
        input,
        kind: InstructionKind::Perform(Box::new(effect)),
        source_effect_path: Some(source_effect_path),
    }
}

fn typed_discard_branch(
    effect: &SpellEffectKind,
    result: &str,
) -> Option<(
    ProgramResultCondition,
    crate::ChoiceId,
    Vec<SpellEffectKind>,
    Vec<SpellEffectKind>,
)> {
    let SpellEffectKind::ChooseResolutionBranch {
        chooser: PlayerRecipient::Controller,
        optional: false,
        selection: super::ResolutionBranchSelection::FirstApplicable,
        branches,
        otherwise,
    } = effect
    else {
        return None;
    };
    if !otherwise.is_empty() {
        return None;
    }
    let mut result_branch = None;
    for branch in branches {
        let ResolutionBranchRequirement::CardResultCount { filter, min, max } = &branch.requirement
        else {
            if branch.requirement == ResolutionBranchRequirement::Always
                && branch.cost == super::ResolutionCost::None
                && branch.effects.is_empty()
            {
                continue;
            }
            return None;
        };
        if filter.source != super::CardResultSource::PreviousEffect
            || filter.action != super::CardResultAction::Discard
            || filter.players != RelativePlayerSet::Controller
            || filter.card_type != Some(CardTypeFilter::Nonland)
            || branch.cost != super::ResolutionCost::None
            || branch.effects.is_empty()
            || result_branch.is_some()
        {
            return None;
        }
        result_branch = Some((
            filter,
            *min,
            *max,
            branch.branch_id.clone(),
            branch.effects.clone(),
        ));
    }
    let (filter, min, max, branch_id, effects) = result_branch?;
    Some((
        ProgramResultCondition::DiscardCount {
            result: result.to_owned(),
            players: filter.players,
            card_type: filter.card_type,
            min,
            max,
        },
        branch_id,
        effects,
        Vec::new(),
    ))
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
        }
    )
}

fn effect_contains_draw_discard(effect: &SpellEffectKind) -> bool {
    match effect {
        SpellEffectKind::DrawDiscard { .. } => true,
        SpellEffectKind::Conditional { effect, .. }
        | SpellEffectKind::ConditionalCastCost { effect, .. } => {
            effect_contains_draw_discard(effect)
        }
        SpellEffectKind::ChooseResolutionBranch {
            branches,
            otherwise,
            ..
        } => branches
            .iter()
            .flat_map(|branch| &branch.effects)
            .chain(otherwise)
            .any(effect_contains_draw_discard),
        SpellEffectKind::CreateReflexiveTrigger { ability, .. } => {
            ability.effect.iter().any(effect_contains_draw_discard)
        }
        SpellEffectKind::GrantTriggeredAbility { ability, .. }
        | SpellEffectKind::CreateDelayedTrigger { ability, .. } => {
            ability.effect.iter().any(effect_contains_draw_discard)
        }
        SpellEffectKind::ApplyPermanentModifier {
            modifier: super::ResolvingPermanentModifier::GrantActivatedAbility(ability),
            ..
        } => ability.effect.iter().any(effect_contains_draw_discard),
        _ => false,
    }
}

pub(crate) fn effects_contain_draw_discard(effects: &[SpellEffectKind]) -> bool {
    effects.iter().any(effect_contains_draw_discard)
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
            let matches = matches!(
                (&actual.result_type, expected),
                (ResultType::PermanentSelection { .. }, "PermanentSelection")
                    | (
                        ResultType::HandSelection { ordered: true, .. },
                        "HandSelection"
                    )
            );
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
