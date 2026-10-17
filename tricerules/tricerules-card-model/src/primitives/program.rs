//! Finite, typed resolution programs for cards whose ordered actions need explicit results.
//!
//! Programs reuse the ordinary effect vocabulary for leaf actions. Their compiler gives results
//! stable slots and requires every branch to declare the values it yields to its parent.
//! The initial runner is deliberately narrower than that vocabulary: target groups bind only the
//! finite authored role signatures compiled below, while the remaining supported instructions
//! are controller-scoped spell/ability sequences. Draw and private hand-choice amounts are fixed
//! literals, `If` accepts only controller-scoped hand/library counts, graveyard CardCount filters,
//! and battlefield Count filters, and permanent choices cannot use event-bound filters or
//! cross-object constraints. Widening these rules needs a separately typed context/result contract.

use super::{
    Amount, CardResultAction, CardSearchZone, CardTypeFilter, CastCostGroupDef,
    CastCostReceiptCondition, ChooseCardSource, CohortPartitionSelection, EffectContext,
    EffectSubject, GameCondition, LifeAmount, PlayerRecipient, RelativePlayerSet,
    ResolutionBranchRequirement, ResolutionCost, SearchDestination, SearchSelectionBounds,
    SearchSelectionConstraint, SearchZoneSelection, SpellEffectKind, TargetFilter, TargetKind,
    TargetSchema, TargetingDef, TokenCopySource, ZoneCardFilter,
};
use crate::{choice_fallback, AbilityPresentation, ChoiceId, ModeId, ProgramOwnerId};
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
    TriggerObservedObject,
    /// Exact object reference captured by the structurally admitted delayed observer. Unlike an
    /// observed-object snapshot, this reference may be stale or absent from its former zone when
    /// the delayed ability resolves; only the captured identity must have been supplied.
    CapturedTriggerObjectRef,
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
    Choose {
        choice: ChoiceId,
        chooser: PlayerRecipient,
        optional: bool,
        branches: Vec<AuthoredChooseBranch>,
        otherwise: AuthoredBlock,
    },
}

/// One player-selected branch in an authored `Choose` control-flow node. Its body is a scoped
/// program block, not an adapter around the legacy flat effect list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredChooseBranch {
    pub branch_id: ChoiceId,
    pub presentation: AbilityPresentation,
    pub cost: ResolutionCost,
    #[serde(default)]
    pub requirement: ResolutionBranchRequirement,
    pub body: AuthoredBlock,
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
    CounterPayment {
        result: String,
        outcome: CounterPaymentOutcome,
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

/// The payment state for a fixed-cost soft-counter instruction. `ForcedUnpaid` records that an
/// already-offered payer left the game before answering; `NotOffered` records a completed
/// instruction that found no current captured stack target, including when an earlier instruction
/// removed it, or a skipped target group while another legal group keeps the program resolving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CounterPaymentOutcome {
    NotOffered,
    Declined,
    Paid,
    ForcedUnpaid,
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
    CounterPayment {
        result: ResultSlotId,
        outcome: CounterPaymentOutcome,
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
    CounterPaymentOutcome,
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
    FoundLibraryCohort,
    RevealedLibraryCohort,
    CardPartition,
    /// Exact replacement-expanded TriggerObject token cohort from one admitted copy effect.
    TokenCopyBatch,
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
    Choose {
        choice: ChoiceId,
        chooser: PlayerRecipient,
        optional: bool,
        branches: Vec<CompiledChooseBranch>,
        otherwise: CompiledBlock,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledChooseBranch {
    pub branch_id: ChoiceId,
    pub presentation: AbilityPresentation,
    pub cost: ResolutionCost,
    pub requirement: CompiledChooseRequirement,
    pub body: CompiledBlock,
}

impl CompiledChooseBranch {
    /// Render the same public fallback wording used by the existing resolution-branch contract.
    /// Branch and instruction IDs remain internal selectors for program control flow.
    pub fn fallback_label(&self) -> String {
        match &self.cost {
            ResolutionCost::Waterbend(cost) => format!("Waterbend {cost}"),
            ResolutionCost::Mana(cost) => format!("Pay {cost}"),
            ResolutionCost::None if self.body.steps.is_empty() => "Continue".into(),
            ResolutionCost::None => {
                let effects = self
                    .body
                    .steps
                    .iter()
                    .map(|instruction| match &instruction.kind {
                        CompiledInstructionKind::Perform(effect) => Some((**effect).clone()),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>();
                effects
                    .as_deref()
                    .and_then(super::presentation::simple_effects)
                    .unwrap_or_else(|| choice_fallback("Choice", &self.branch_id))
            }
            _ => choice_fallback("Choice", &self.branch_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompiledChooseRequirement {
    Always,
    GameCondition(GameCondition),
    CastCostReceipt(CastCostReceiptCondition),
    /// The exact source object can receive one +1/+1 counter at this resolution point.
    SourceCanReceivePlusOnePlusOneCounters,
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
    cast_cost_options: HashMap<ChoiceId, HashSet<ChoiceId>>,
    slots: Vec<CompiledSlot>,
    instruction_ids: HashSet<String>,
    choice_ids: HashSet<ChoiceId>,
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
        Self::compile_with_targeting_and_cast_cost_groups(owner, context, program, targeting, &[])
    }

    /// Compile one exact program owner with the cast-cost identities available to that owner.
    /// Face modes share their parent face's cast-cost groups, but never borrow a sibling face's
    /// receipt namespace. Ability programs pass their source face's groups and reject spell-only
    /// receipt predicates below.
    pub(crate) fn compile_with_targeting_and_cast_cost_groups(
        owner: ProgramOwnerId,
        context: EffectContext,
        program: &CardProgram,
        targeting: Option<&TargetingDef>,
        cast_cost_groups: &[CastCostGroupDef],
    ) -> Result<Self, String> {
        let mut compiled = Self::compile_internal(owner, context, program, cast_cost_groups)?;
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
        cast_cost_groups: &[CastCostGroupDef],
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
            cast_cost_options: cast_cost_groups
                .iter()
                .map(|group| {
                    (
                        group.group_id.clone(),
                        group
                            .options
                            .iter()
                            .map(|option| option.option_id().clone())
                            .collect(),
                    )
                })
                .collect(),
            slots: Vec::new(),
            instruction_ids: HashSet::new(),
            choice_ids: HashSet::new(),
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
        )
        | (
            SpellEffectKind::ChooseCohortPartition {
                chooser: PlayerRecipient::TargetedPlayer { .. },
                ..
            },
            ProgramTargetRole::Target,
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
        match &instruction.kind {
            CompiledInstructionKind::If { yes, no, .. } => {
                if let Some(found) = find_instruction_by_id(yes, id) {
                    return Some(found);
                }
                if let Some(found) = find_instruction_by_id(no, id) {
                    return Some(found);
                }
            }
            CompiledInstructionKind::Choose {
                branches,
                otherwise,
                ..
            } => {
                for branch in branches {
                    if let Some(found) = find_instruction_by_id(&branch.body, id) {
                        return Some(found);
                    }
                }
                if let Some(found) = find_instruction_by_id(otherwise, id) {
                    return Some(found);
                }
            }
            CompiledInstructionKind::Perform(_) => {}
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
                    if matches!(
                        effect.as_ref(),
                        SpellEffectKind::CreateTokenCopies {
                            source: TokenCopySource::TriggerObject,
                            ..
                        }
                    ) {
                        self.required_inputs
                            .insert(ProgramRequiredInput::TriggerObservedObject);
                    }
                    if matches!(
                        effect.as_ref(),
                        SpellEffectKind::ReturnTriggeredCard {
                            reference: super::TriggeredCardReference::ExactTriggerObject,
                            ..
                        }
                    ) {
                        self.required_inputs
                            .insert(ProgramRequiredInput::CapturedTriggerObjectRef);
                    }
                    if (matches!(
                        effect.as_ref(),
                        SpellEffectKind::SearchLibrary {
                            selection_bounds: Some(_),
                            ..
                        } if !is_resultless_direct_top_library_search(effect)
                            && !is_resultless_direct_hand_library_search(effect)
                            && !is_resultless_direct_battlefield_library_search(effect)
                            && !is_resultless_embermouth_conditional_top_search(effect)
                    ) || matches!(
                        effect.as_ref(),
                        SpellEffectKind::RevealCohort
                            | SpellEffectKind::ChooseCohortPartition { .. }
                    )) && instruction.bind.len() != 1
                    {
                        return Err(format!(
                            "instruction '{}' must bind its typed cohort result",
                            instruction.id
                        ));
                    }
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
                InstructionKind::Choose {
                    choice,
                    chooser,
                    optional,
                    branches,
                    otherwise,
                } => {
                    if instruction.input.is_some() {
                        return Err(format!(
                            "Choose instruction '{}' cannot consume an input result",
                            instruction.id
                        ));
                    }
                    choice.validate().map_err(|reason| {
                        format!("Choose instruction '{}': {reason}", instruction.id)
                    })?;
                    if !self.choice_ids.insert(choice.clone()) {
                        return Err(format!(
                            "duplicate Choose choice id '{}' in program",
                            choice
                        ));
                    }
                    let chooser_supported = *chooser == PlayerRecipient::Controller
                        || (*chooser == PlayerRecipient::SourceController
                            && self.context == EffectContext::Ability);
                    if !chooser_supported {
                        return Err(format!(
                            "Choose instruction '{}' supports Controller, or SourceController on an ability program, as chooser",
                            instruction.id
                        ));
                    }
                    if branches.is_empty() {
                        return Err(format!(
                            "Choose instruction '{}' requires at least one branch",
                            instruction.id
                        ));
                    }

                    let mut branch_ids = HashSet::new();
                    let mut has_free_branch = false;
                    let mut has_paid_branch = false;
                    let mut compiled_branches = Vec::with_capacity(branches.len());
                    let mut joined_types: Option<Vec<ResultType>> = None;
                    for branch in branches {
                        branch.branch_id.validate().map_err(|reason| {
                            format!("Choose instruction '{}' branch: {reason}", instruction.id)
                        })?;
                        if !branch_ids.insert(branch.branch_id.clone()) {
                            return Err(format!(
                                "Choose instruction '{}' has duplicate branch id '{}'",
                                instruction.id, branch.branch_id
                            ));
                        }
                        branch.presentation.validate().map_err(|reason| {
                            format!(
                                "Choose instruction '{}' branch '{}': {reason}",
                                instruction.id, branch.branch_id
                            )
                        })?;
                        match &branch.cost {
                            ResolutionCost::None => has_free_branch = true,
                            ResolutionCost::Mana(cost) | ResolutionCost::Waterbend(cost) => {
                                has_paid_branch = true;
                                if cost.pips.is_empty()
                                    || cost
                                        .pips
                                        .iter()
                                        .any(|pip| matches!(pip, crate::ManaSymbol::X))
                                {
                                    return Err(format!(
                                        "Choose instruction '{}' branch '{}' requires a nonempty fixed mana cost without X",
                                        instruction.id, branch.branch_id
                                    ));
                                }
                            }
                            unsupported => {
                                return Err(format!(
                                    "Choose instruction '{}' does not support resolution cost {unsupported:?}",
                                    instruction.id
                                ));
                            }
                        }

                        let requirement = match &branch.requirement {
                            ResolutionBranchRequirement::Always => {
                                CompiledChooseRequirement::Always
                            }
                            ResolutionBranchRequirement::GameCondition(condition) => {
                                let compiled = compile_program_condition(
                                    &ProgramCondition::Game(condition.clone()),
                                    &names,
                                    &self.slots,
                                )
                                .map_err(|reason| {
                                    format!(
                                        "Choose instruction '{}' branch '{}': {reason}",
                                        instruction.id, branch.branch_id
                                    )
                                })?;
                                let CompiledProgramCondition::Game(condition) = compiled else {
                                    unreachable!(
                                        "game branch requirement compiles as a game condition"
                                    )
                                };
                                CompiledChooseRequirement::GameCondition(condition)
                            }
                            ResolutionBranchRequirement::CastCostReceipt(condition) => {
                                if self.context != EffectContext::Spell {
                                    return Err(format!(
                                        "Choose instruction '{}' cast-cost requirements require a spell program owner",
                                        instruction.id
                                    ));
                                }
                                let Some(options) = self.cast_cost_options.get(&condition.group_id)
                                else {
                                    return Err(format!(
                                        "Choose instruction '{}' references an unknown owning cast-cost group",
                                        instruction.id
                                    ));
                                };
                                if !options.contains(&condition.option_id) {
                                    return Err(format!(
                                        "Choose instruction '{}' references an unknown option in its owning cast-cost group",
                                        instruction.id
                                    ));
                                }
                                CompiledChooseRequirement::CastCostReceipt(condition.clone())
                            }
                            ResolutionBranchRequirement::EffectsApplicable => {
                                if self.context != EffectContext::Ability
                                    || !exact_source_plus_one_counter_body(&branch.body)
                                {
                                    return Err(format!(
                                        "Choose instruction '{}' supports EffectsApplicable only for an ability branch containing one resultless fixed positive Source +1/+1 PutCounters instruction",
                                        instruction.id
                                    ));
                                }
                                CompiledChooseRequirement::SourceCanReceivePlusOnePlusOneCounters
                            }
                            ResolutionBranchRequirement::CardResultCount { .. } => {
                                return Err(format!(
                                    "Choose instruction '{}' does not support CardResultCount requirements",
                                    instruction.id
                                ));
                            }
                        };

                        let (body, body_types) = self.compile_block(&branch.body, &names)?;
                        joined_types = Some(match joined_types {
                            None => body_types,
                            Some(current) => {
                                join_choose_yield_types(&instruction.id, current, body_types)?
                            }
                        });
                        compiled_branches.push(CompiledChooseBranch {
                            branch_id: branch.branch_id.clone(),
                            presentation: branch.presentation.clone(),
                            cost: branch.cost.clone(),
                            requirement,
                            body,
                        });
                    }
                    if has_free_branch && has_paid_branch {
                        return Err(format!(
                            "Choose instruction '{}' cannot mix free and paid sibling branches",
                            instruction.id
                        ));
                    }

                    let (otherwise, otherwise_types) = self.compile_block(otherwise, &names)?;
                    let result_types = join_choose_yield_types(
                        &instruction.id,
                        joined_types.unwrap_or_default(),
                        otherwise_types,
                    )?;
                    (
                        None,
                        result_types,
                        CompiledInstructionKind::Choose {
                            choice: choice.clone(),
                            chooser: *chooser,
                            optional: *optional,
                            branches: compiled_branches,
                            otherwise,
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
        ProgramCondition::Result(ProgramResultCondition::CounterPayment { result, outcome }) => {
            let slot = result_slot(result, names, slots)?;
            if !matches!(
                &slots[slot.0 as usize].result_type,
                ResultType::CounterPaymentOutcome
            ) {
                return Err(format!(
                    "result '{result}' is not a compatible counter-payment result"
                ));
            }
            Ok(CompiledProgramCondition::Result(
                CompiledProgramResultCondition::CounterPayment {
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
    let token_copy_batch_consumer = is_token_copy_batch_consumer(effect);
    let expected = match effect {
        EffectOp::UntapChosenPermanents => Some("PermanentSelection"),
        EffectOp::PutSelectedHandCardsOnLibraryTop => Some("HandSelection"),
        EffectOp::ChooseCard {
            source: ChooseCardSource::CardResultCohort { .. },
            ..
        } => Some("CardResultCohort"),
        EffectOp::RevealCohort => Some("FoundLibraryCohort"),
        EffectOp::ChooseCohortPartition { .. } => Some("RevealedLibraryCohort"),
        EffectOp::MoveCohortPartition { .. } => Some("CardPartition"),
        _ if token_copy_batch_consumer => Some("TokenCopyBatch"),
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
                EffectOp::RevealCohort => {
                    matches!(actual.result_type, ResultType::FoundLibraryCohort)
                }
                EffectOp::ChooseCohortPartition { .. } => {
                    matches!(actual.result_type, ResultType::RevealedLibraryCohort)
                }
                EffectOp::MoveCohortPartition { .. } => {
                    matches!(actual.result_type, ResultType::CardPartition)
                }
                _ if token_copy_batch_consumer => {
                    matches!(actual.result_type, ResultType::TokenCopyBatch)
                }
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
        (ResultType::CounterPaymentOutcome, ResultType::CounterPaymentOutcome) => {
            ResultType::CounterPaymentOutcome
        }
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
        (ResultType::FoundLibraryCohort, ResultType::FoundLibraryCohort) => {
            ResultType::FoundLibraryCohort
        }
        (ResultType::RevealedLibraryCohort, ResultType::RevealedLibraryCohort) => {
            ResultType::RevealedLibraryCohort
        }
        (ResultType::CardPartition, ResultType::CardPartition) => ResultType::CardPartition,
        (ResultType::TokenCopyBatch, ResultType::TokenCopyBatch) => ResultType::TokenCopyBatch,
        _ => return None,
    })
}

fn join_choose_yield_types(
    instruction_id: &str,
    left: Vec<ResultType>,
    right: Vec<ResultType>,
) -> Result<Vec<ResultType>, String> {
    if left.len() != right.len() {
        return Err(format!(
            "Choose instruction '{instruction_id}' has mismatched explicit yield arity"
        ));
    }
    left.into_iter()
        .zip(right)
        .map(|(left, right)| {
            join_result_types(&left, &right).ok_or_else(|| {
                format!("Choose instruction '{instruction_id}' has mismatched explicit yield types")
            })
        })
        .collect()
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
        SpellEffectKind::GainLife { amount } => {
            effect.validate(context)?;
            validate_program_amount(amount)?;
            validate_fixed_program_amount("GainLife amount", amount)?;
            Ok(Vec::new())
        }
        SpellEffectKind::LoseLife { amount, who }
            if *who == PlayerRecipient::EachOpponent && matches!(amount, LifeAmount::Fixed(_)) =>
        {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::LoseLife { .. } => Err(
            "LoseLife in a resolution program supports only a fixed amount for EachOpponent".into(),
        ),
        SpellEffectKind::Blight { .. } => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::Proliferate => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::CreateReflexiveTrigger { .. } => {
            effect.validate(context)?;
            validate_resultless_program_reflexive_trigger(effect)?;
            Ok(Vec::new())
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
        SpellEffectKind::SearchLibrary {
            selection_bounds: Some(_),
            destination: SearchDestination::TopOfLibrary,
            conditional_destination: Some(_),
            ..
        } => validate_resultless_embermouth_conditional_top_search(effect, context),
        SpellEffectKind::SearchLibrary {
            selection_bounds: Some(_),
            destination: SearchDestination::TopOfLibrary,
            ..
        } => validate_resultless_direct_top_library_search(effect, context),
        SpellEffectKind::SearchLibrary {
            selection_bounds: Some(_),
            destination: SearchDestination::Hand,
            ..
        } => validate_resultless_direct_hand_library_search(effect, context),
        SpellEffectKind::SearchLibrary {
            selection_bounds: Some(_),
            destination: SearchDestination::Battlefield { .. },
            ..
        } => validate_resultless_direct_battlefield_library_search(effect, context),
        SpellEffectKind::SearchLibrary {
            selection_bounds: Some(_),
            ..
        } => validate_retained_library_search(effect, context),
        SpellEffectKind::SearchLibrary { .. } => {
            Err("SearchLibrary in spell_program requires typed retained selection_bounds".into())
        }
        SpellEffectKind::RevealCohort => Ok(vec![ResultType::RevealedLibraryCohort]),
        SpellEffectKind::ChooseCohortPartition { chooser, selection } => {
            let PlayerRecipient::TargetedPlayer { kind, .. } = chooser else {
                return Err(
                    "ChooseCohortPartition requires a targeted player chooser in spell_program"
                        .into(),
                );
            };
            if *kind != TargetKind::OpponentPlayer {
                return Err(
                    "ChooseCohortPartition chooser must target an opponent in spell_program".into(),
                );
            }
            match selection {
                CohortPartitionSelection::ExactlyUpTo(limit) if *limit > 0 => {}
                CohortPartitionSelection::ExactlyUpTo(_) => {
                    return Err("ChooseCohortPartition ExactlyUpTo limit must be positive".into())
                }
            }
            Ok(vec![ResultType::CardPartition])
        }
        SpellEffectKind::MoveCohortPartition {
            selected_to,
            remainder_to,
        } => {
            let valid_destination = |destination| {
                matches!(
                    destination,
                    SearchDestination::Hand | SearchDestination::Graveyard
                )
            };
            if !valid_destination(*selected_to)
                || !valid_destination(*remainder_to)
                || selected_to == remainder_to
            {
                return Err(
                    "MoveCohortPartition requires distinct Hand or Graveyard destinations".into(),
                );
            }
            Ok(Vec::new())
        }
        SpellEffectKind::ShuffleLibrary { who } => {
            validate_program_controller(*who, "ShuffleLibrary recipient")?;
            Ok(Vec::new())
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
            Ok(if unless_controller_pays.is_some() {
                vec![ResultType::CounterPaymentOutcome]
            } else {
                Vec::new()
            })
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
        SpellEffectKind::ReturnAllToOwnersHand { .. } => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::CreateTokens {
            count,
            who,
            sacrifice_timing,
            ..
        } => {
            effect.validate(context)?;
            let supported_recipient = *who == PlayerRecipient::Controller
                || (*who == PlayerRecipient::SourceController && context == EffectContext::Ability);
            if !supported_recipient || sacrifice_timing.is_some() {
                return Err(
                    "CreateTokens supports controller-owned ordinary tokens, or SourceController tokens on an ability program, without delayed sacrifice"
                        .into(),
                );
            }
            validate_program_amount(count)?;
            validate_fixed_program_amount("CreateTokens count", count)?;
            Ok(Vec::new())
        }
        SpellEffectKind::CreateTokenCopies { count, source } => {
            effect.validate(context)?;
            validate_program_amount(count)?;
            validate_fixed_program_amount("CreateTokenCopies count", count)?;
            match source {
                super::TokenCopySource::Chosen(filter) => {
                    validate_program_target_filter(filter, "CreateTokenCopies target")?;
                }
                super::TokenCopySource::TriggerObject if context == EffectContext::Ability => {}
                super::TokenCopySource::TriggerObject => {
                    return Err(
                        "TriggerObject token copies require an ability program owner".into(),
                    );
                }
                super::TokenCopySource::Source | super::TokenCopySource::LinkedExile { .. } => {
                    return Err(
                        "CreateTokenCopies source is not supported in a resolution program".into(),
                    );
                }
            }
            Ok(if matches!(source, TokenCopySource::TriggerObject) {
                vec![ResultType::TokenCopyBatch]
            } else {
                Vec::new()
            })
        }
        SpellEffectKind::PutCounters {
            counter,
            count,
            subject,
        } if *counter == super::CounterKind::PlusOnePlusOne
            && matches!(count, Amount::Fixed(amount) if *amount > 0)
            && *subject == EffectSubject::Source
            && context == EffectContext::Ability =>
        {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::ReturnTriggeredCard {
            reference: super::TriggeredCardReference::ExactTriggerObject,
            from,
            destination: super::TriggeredCardDestination::Battlefield,
            tapped: false,
            controller: super::ReturnController::Owner,
            entry_counters,
            set_types: None,
        } if context == EffectContext::Ability
            && !from.is_empty()
            && from.as_slice() == [super::EventZone::Exile]
            && entry_counters.is_empty() =>
        {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::ApplyPermanentModifier { .. } if is_token_copy_batch_consumer(effect) => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::CreateDelayedTrigger { .. } if is_token_copy_batch_consumer(effect) => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        _ => Err("effect is not in the resolution-program operation whitelist".into()),
    }
}

/// The only program consumers of a TriggerObject copy batch are the two exact Flameshadow /
/// Molten Echoes follow-ups. Keeping the input contract on the whole operation shape prevents a
/// typed slot from silently widening PreviousEffectObjects to arbitrary effect families.
fn is_token_copy_batch_consumer(effect: &SpellEffectKind) -> bool {
    match effect {
        SpellEffectKind::ApplyPermanentModifier {
            subject: EffectSubject::PreviousEffectObjects,
            modifier: super::ResolvingPermanentModifier::GrantKeywords(keywords),
            duration: super::ResolvingEffectDuration::Indefinite,
        } => keywords.as_slice() == [super::Keyword::Haste],
        SpellEffectKind::CreateDelayedTrigger {
            subject: Some(EffectSubject::PreviousEffectObjects),
            affected_player: None,
            ability,
        } => {
            matches!(
                ability.trigger,
                super::TriggerCondition::AtBeginningOfNextEndStep
            ) && ability.effect.as_slice() == [SpellEffectKind::ExileObservedObjects]
                && ability.program.is_none()
                && ability.modal.is_none()
                && ability.targeting.is_none()
                && !ability.may
                && ability.intervening_if.is_none()
                && !ability.triggers_only_once
                && ability.max_triggers_per_turn.is_none()
        }
        _ => false,
    }
}

fn validate_resultless_program_reflexive_trigger(effect: &SpellEffectKind) -> Result<(), String> {
    let SpellEffectKind::CreateReflexiveTrigger { when, ability } = effect else {
        return Err("expected a CreateReflexiveTrigger program leaf".into());
    };
    if when.is_some() || ability.targeting.is_some() || ability.intervening_if.is_some() {
        return Err(
            "CreateReflexiveTrigger supports only the approved resultless target shapes".into(),
        );
    }

    let sentry_filter = TargetFilter {
        kind: TargetKind::Creature,
        controller: super::TargetController::Opponent,
        ..TargetFilter::default()
    };
    if let [SpellEffectKind::Tap {
        subject: EffectSubject::Chosen(filter),
    }] = ability.effect.as_slice()
    {
        if filter.as_ref() == &sentry_filter {
            return Ok(());
        }
    }

    if let [SpellEffectKind::DamageTarget {
        amount: Amount::Fixed(3),
        target,
    }] = ability.effect.as_slice()
    {
        if target == &TargetFilter::default() {
            return Ok(());
        }
    }

    Err("CreateReflexiveTrigger supports only the approved resultless target shapes".into())
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

fn validate_retained_library_search(
    effect: &SpellEffectKind,
    context: EffectContext,
) -> Result<Vec<ResultType>, String> {
    let SpellEffectKind::SearchLibrary {
        who,
        optional,
        count,
        count_by_cast_cost,
        filter,
        selection_constraint,
        slots,
        zones,
        destination,
        conditional_destination,
        shuffle,
        reveal,
        result_id,
        selection_bounds: Some(SearchSelectionBounds { min, max }),
    } = effect
    else {
        return Err("SearchLibrary requires typed retained selection_bounds".into());
    };
    if *count == 0 || min > max || *max != *count {
        return Err(
            "retained SearchLibrary bounds must satisfy 0 <= min <= max == count and count > 0"
                .into(),
        );
    }
    if *who != PlayerRecipient::Controller
        || *optional
        || count_by_cast_cost.is_some()
        || filter.is_some()
        || !matches!(
            selection_constraint,
            None | Some(SearchSelectionConstraint::DifferentNames)
        )
        || !slots.is_empty()
        || *zones != SearchZoneSelection::Fixed(vec![CardSearchZone::Library])
        || *destination != SearchDestination::KeepInLibrary
        || conditional_destination.is_some()
        || *shuffle
        || *reveal
        || result_id.is_some()
    {
        return Err(
            "retained SearchLibrary supports only a mandatory controller library search with fixed bounds, optional DifferentNames, and KeepInLibrary"
                .into(),
        );
    }

    // Reuse the ordinary direct-search validator for its shared positive-count and zone rules,
    // removing only the program-owned fields it intentionally rejects.
    let mut ordinary = effect.clone();
    if let SpellEffectKind::SearchLibrary {
        selection_bounds,
        selection_constraint,
        destination,
        ..
    } = &mut ordinary
    {
        *selection_bounds = None;
        if *selection_constraint == Some(SearchSelectionConstraint::DifferentNames) {
            *selection_constraint = None;
        }
        *destination = SearchDestination::Hand;
    }
    ordinary.validate(context)?;
    Ok(vec![ResultType::FoundLibraryCohort])
}

fn is_resultless_direct_search_base(effect: &SpellEffectKind) -> bool {
    let SpellEffectKind::SearchLibrary {
        who,
        optional,
        count,
        count_by_cast_cost,
        selection_constraint,
        slots,
        zones,
        conditional_destination,
        shuffle,
        result_id,
        selection_bounds,
        ..
    } = effect
    else {
        return false;
    };

    *who == PlayerRecipient::Controller
        && !*optional
        && *count == 1
        && count_by_cast_cost.is_none()
        && selection_constraint.is_none()
        && slots.is_empty()
        && *zones == SearchZoneSelection::Fixed(vec![CardSearchZone::Library])
        && conditional_destination.is_none()
        && *shuffle
        && result_id.is_none()
        && *selection_bounds == Some(SearchSelectionBounds { min: 0, max: 1 })
}

fn is_resultless_direct_top_library_search(effect: &SpellEffectKind) -> bool {
    let SpellEffectKind::SearchLibrary {
        filter,
        destination,
        reveal,
        ..
    } = effect
    else {
        return false;
    };

    is_resultless_direct_search_base(effect)
        && filter.as_ref().is_some_and(|filter| {
            *filter
                == ZoneCardFilter {
                    card_type: Some(CardTypeFilter::BasicLand),
                    ..ZoneCardFilter::default()
                }
        })
        && *destination == SearchDestination::TopOfLibrary
        && *reveal
}

fn validate_resultless_direct_top_library_search(
    effect: &SpellEffectKind,
    context: EffectContext,
) -> Result<Vec<ResultType>, String> {
    if !is_resultless_direct_top_library_search(effect) {
        return Err(
            "program direct TopOfLibrary SearchLibrary supports only a resultless controller BasicLand search with fixed count 1, bounds 0..1, reveal and shuffle"
                .into(),
        );
    }

    validate_resultless_direct_search(effect, context)
}

fn is_resultless_direct_hand_library_search(effect: &SpellEffectKind) -> bool {
    let SpellEffectKind::SearchLibrary {
        filter,
        destination,
        reveal,
        ..
    } = effect
    else {
        return false;
    };

    let permitted_filter = filter.as_ref().is_some_and(|filter| {
        if filter.validate().is_err() {
            return false;
        }
        let basic_land = filter
            == &ZoneCardFilter {
                card_type: Some(CardTypeFilter::BasicLand),
                ..ZoneCardFilter::default()
            };
        let exact_name_only = filter.exact_name.as_ref().is_some_and(|name| {
            !name.trim().is_empty()
                && filter
                    == &ZoneCardFilter {
                        exact_name: Some(name.clone()),
                        ..ZoneCardFilter::default()
                    }
        });
        basic_land || exact_name_only
    });

    is_resultless_direct_search_base(effect)
        && permitted_filter
        && *destination == SearchDestination::Hand
        && *reveal
}

fn validate_resultless_direct_hand_library_search(
    effect: &SpellEffectKind,
    context: EffectContext,
) -> Result<Vec<ResultType>, String> {
    if !is_resultless_direct_hand_library_search(effect) {
        return Err(
            "program direct Hand SearchLibrary supports only a resultless controller BasicLand or exact-name search with fixed count 1, bounds 0..1, reveal and shuffle"
                .into(),
        );
    }

    // The authored bounds are consumed by this one-card hand movement. The normal direct effect
    // validator still checks the actor, filter, zone, count, and destination contracts.
    validate_resultless_direct_search(effect, context)
}

fn is_resultless_direct_battlefield_library_search(effect: &SpellEffectKind) -> bool {
    let SpellEffectKind::SearchLibrary {
        filter,
        destination,
        reveal,
        ..
    } = effect
    else {
        return false;
    };

    is_resultless_direct_search_base(effect)
        && filter.as_ref().is_some_and(|filter| {
            *filter
                == ZoneCardFilter {
                    card_type: Some(CardTypeFilter::BasicLand),
                    ..ZoneCardFilter::default()
                }
        })
        && *destination == (SearchDestination::Battlefield { tapped: true })
        && !*reveal
}

fn validate_resultless_direct_battlefield_library_search(
    effect: &SpellEffectKind,
    context: EffectContext,
) -> Result<Vec<ResultType>, String> {
    if !is_resultless_direct_battlefield_library_search(effect) {
        return Err(
            "program direct Battlefield SearchLibrary supports only a resultless controller BasicLand search with fixed count 1, bounds 0..1, tapped destination, no reveal, and shuffle"
                .into(),
        );
    }

    validate_resultless_direct_search(effect, context)
}

fn validate_resultless_direct_search(
    effect: &SpellEffectKind,
    context: EffectContext,
) -> Result<Vec<ResultType>, String> {
    // Program selection bounds belong to this typed resultless operation. Normalize only that
    // Program-owned field before applying the ordinary SearchLibrary validator.
    let mut ordinary = effect.clone();
    if let SpellEffectKind::SearchLibrary {
        selection_bounds, ..
    } = &mut ordinary
    {
        *selection_bounds = None;
    }
    ordinary.validate(context)?;
    Ok(Vec::new())
}

/// Keep Embermouth Sentinel's conditional destination as a closed, resultless program leaf.
/// The shared BattlefieldAggregate condition must remain the exact live "controller controls a
/// Dragon" predicate; in particular, do not narrow it to creatures, which would omit Kindreds.
fn validate_resultless_embermouth_conditional_top_search(
    effect: &SpellEffectKind,
    context: EffectContext,
) -> Result<Vec<ResultType>, String> {
    if !is_resultless_embermouth_conditional_top_search(effect) {
        return Err(
            "program conditional TopOfLibrary SearchLibrary supports only Embermouth's resultless controller BasicLand search with the exact live Controller Dragon-count to tapped Battlefield destination, fixed count 1, bounds 0..1, reveal and shuffle"
                .into(),
        );
    }

    // Only Program-owned bounds are normalized away; all ordinary SearchLibrary validation
    // remains authoritative for this otherwise standard search.
    validate_resultless_direct_search(effect, context)
}

fn is_resultless_embermouth_conditional_top_search(effect: &SpellEffectKind) -> bool {
    let SpellEffectKind::SearchLibrary {
        who,
        optional,
        count,
        count_by_cast_cost,
        filter,
        selection_constraint,
        slots,
        zones,
        destination,
        conditional_destination,
        shuffle,
        reveal,
        result_id,
        selection_bounds,
    } = effect
    else {
        return false;
    };
    let basic_land = filter.as_ref().is_some_and(|filter| {
        filter
            == &ZoneCardFilter {
                card_type: Some(CardTypeFilter::BasicLand),
                ..ZoneCardFilter::default()
            }
    });
    let condition = super::GameCondition::BattlefieldAggregate {
        filter: super::BattlefieldPermanentFilter {
            required_supertypes: Vec::new(),
            token: None,
            any_of: None,
            controllers: super::RelativePlayerSet::Controller,
            card_type: None,
            color: None,
            name: None,
            required_subtypes: vec!["Dragon".into()],
            exclude_source: false,
        },
        aggregate: super::BattlefieldAggregate::Count,
        min: Some(1),
        max: None,
        exclude_observed_object: false,
    };
    let exact_conditional_destination = conditional_destination.as_ref().is_some_and(|actual| {
        actual.condition == condition
            && actual.destination == (SearchDestination::Battlefield { tapped: true })
    });
    *who == PlayerRecipient::Controller
        && !*optional
        && *count == 1
        && count_by_cast_cost.is_none()
        && basic_land
        && selection_constraint.is_none()
        && slots.is_empty()
        && *zones == SearchZoneSelection::Fixed(vec![CardSearchZone::Library])
        && *destination == SearchDestination::TopOfLibrary
        && exact_conditional_destination
        && *shuffle
        && *reveal
        && result_id.is_none()
        && *selection_bounds == Some(SearchSelectionBounds { min: 0, max: 1 })
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

fn exact_source_plus_one_counter_body(body: &AuthoredBlock) -> bool {
    if !body.yields.is_empty() {
        return false;
    }
    let [instruction] = body.steps.as_slice() else {
        return false;
    };
    if !instruction.bind.is_empty() || instruction.input.is_some() {
        return false;
    }
    let InstructionKind::Perform(effect) = &instruction.kind else {
        return false;
    };
    matches!(
        effect.as_ref(),
        SpellEffectKind::PutCounters {
            counter: super::CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(count),
            subject: EffectSubject::Source,
        } if *count > 0
    )
}

fn validate_program_condition(condition: &GameCondition) -> Result<(), String> {
    match condition {
        GameCondition::ControllerLibraryEmpty => {}
        GameCondition::CardsInHand {
            players: super::ConditionPlayerSet::Relative(super::RelativePlayerSet::Controller),
            ..
        } => {}
        GameCondition::GraveyardAggregate {
            owners: RelativePlayerSet::Controller,
            aggregate: super::GraveyardAggregate::CardCount,
            ..
        } => {}
        GameCondition::BattlefieldAggregate {
            filter,
            aggregate: super::BattlefieldAggregate::Count,
            ..
        } => validate_program_battlefield_filter(filter)?,
        GameCondition::AllOf(branches) | GameCondition::AnyOf(branches) => {
            for branch in branches {
                validate_program_condition(branch)?;
            }
        }
        _ => {
            return Err(
                "If condition uses context outside controller hand, library, CardCount graveyard, or Controller battlefield Count state supported by spell_program"
                    .into(),
            )
        }
    }
    condition.validate_live()
}

fn validate_program_battlefield_filter(
    filter: &super::BattlefieldPermanentFilter,
) -> Result<(), String> {
    if filter.controllers != RelativePlayerSet::Controller {
        return Err(
            "ProgramIf battlefield aggregate filters require Controller at every any_of node"
                .into(),
        );
    }
    if let Some(branches) = &filter.any_of {
        for branch in branches {
            validate_program_battlefield_filter(branch)?;
        }
    }
    Ok(())
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
