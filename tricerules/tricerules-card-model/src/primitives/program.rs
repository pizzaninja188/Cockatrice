//! Finite, typed resolution programs for cards whose ordered actions need explicit results.
//!
//! Programs reuse the ordinary effect vocabulary for leaf actions. Their compiler gives results
//! stable slots and requires every branch to declare the values it yields to its parent.
//! The initial runner is deliberately narrower than that vocabulary: target groups bind only the
//! finite authored role signatures compiled below, while the remaining supported instructions
//! are controller-scoped spell/ability sequences. Draw and private hand-choice amounts are fixed
//! literals. `If` admits the global `Void` predicate and, on ability programs, a recursive
//! source-owned condition family built from bounded `SourceCounterCount`, exact live
//! `ObjectMatches(Source, AnyPermanent)`, and nonempty `AllOf`/`AnyOf` composition. Ability Choose
//! requirements additionally admit one exact live controller creature counter-count profile;
//! that profile remains Choose-only because branch draw availability is checked before selection.
//! Conditional search predicates remain narrower.
//! Permanent choices cannot use event-bound filters or cross-object constraints.
//! Widening these rules needs a separately typed context/result contract.

use super::{
    Amount, CardResultAction, CardResultFilter, CardResultSource, CardSearchZone, CardTypeFilter,
    CastCostGroupDef, CastCostReceiptCondition, ChooseCardSource, CohortPartitionSelection,
    CountExpression, EffectContext, EffectSubject, GameCondition, GraveyardDestination,
    LibraryBottomOrder, LifeAmount, PlayerRecipient, QuantityTerm, RelativePlayerSet,
    ResolutionBranchRequirement, ResolutionCost, ResolvingEffectDuration, SearchDestination,
    SearchSelectionBounds, SearchSelectionConstraint, SearchZoneSelection, SpellEffectKind,
    TargetFilter, TargetKind, TargetSchema, TargetingDef, TokenCopySource, ZoneCardFilter,
};
use crate::{
    choice_fallback, AbilityId, AbilityLinkId, AbilityPresentation, ChoiceId, ModeId,
    ProgramOwnerId,
};
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
    StackAbility,
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
    /// Exact event object consumed by the guarded linked-Exile TriggerObject leaf. This remains
    /// distinct from the ETB token-copy snapshot and the delayed exact-reference contract above.
    TriggerLinkedExileObject,
    /// Exact spell or ability reference that chose a permanent carrying a Ward ability. The
    /// captured identity remains present even when that stack object has since left the stack.
    TriggerTargetingStackObject,
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
    ImportCastPayment,
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
#[serde(deny_unknown_fields)]
pub enum ProgramResultCondition {
    CardResultCount {
        result: String,
        #[serde(default)]
        card_type: Option<CardTypeFilter>,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
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
    CounteredAbilitySourceMatches {
        result: String,
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
    CardResultCount {
        result: ResultSlotId,
        card_type: Option<CardTypeFilter>,
        min: Option<u32>,
        max: Option<u32>,
    },
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
    CounteredAbilitySourceMatches {
        result: ResultSlotId,
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
    /// Optional generation-bound source permanent from a successfully countered ability.
    CounteredAbilitySource {
        cardinality: Cardinality,
    },
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
    /// Exact face-down permanent committed by one Manifest Dread instruction. `None` is a
    /// completed empty result; the typed slot remains distinct from an unwritten output.
    ManifestedPermanent {
        cardinality: Cardinality,
    },
    /// Exact post-move object from Koya's optional Exile, carried to its adjacent delayed observer.
    ExiledObject {
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
    /// Exact linked-exile choice facts from one named producer/link pair. The selected card's
    /// owner is carried by its runtime cohort entry, so this origin intentionally has no
    /// `PlayerRecipient`.
    LinkedExileCardResultCohort {
        producer_ability_id: crate::AbilityId,
        linked_exile_id: crate::AbilityLinkId,
        cardinality: Cardinality,
    },
    /// Exact new linked-exile TriggerObject member from Mimic Vat's `imprint` occurrence.
    /// `None` is a meaningful attempted-exile receipt: the paired return instruction still runs.
    TriggerObjectLinkedExileReceipt {
        producer_ability_id: AbilityId,
        linked_exile_id: AbilityLinkId,
        cardinality: Cardinality,
    },
    /// Card-action facts tied to the producer's exact target-group binding, such as chosen
    /// Destroy or targeted-player Discard. Runtime values reuse CardResultCohort.
    TargetCardResultCohort {
        action: CardResultAction,
        cardinality: Cardinality,
        target_group_index: u32,
    },
    /// Exact card objects used to pay the owning spell's one admitted mandatory discard cost.
    /// The payment payer is intentionally not represented as an effect recipient.
    CastPaymentCohort {
        action: CardResultAction,
        cardinality: Cardinality,
    },
    FoundLibraryCohort,
    RevealedLibraryCohort,
    CardPartition,
    /// Exact replacement-expanded TriggerObject token cohort from one admitted copy effect.
    TokenCopyBatch,
    /// Exact replacement-expanded ordinary token cohort from Kyoshi Battle Fan's trigger.
    /// Empty completion is a recorded result, distinct from an unwritten slot.
    CreatedTokenBatch {
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

/// Non-targeting target-group input consumed by a program instruction, such as resolving the
/// controller of a selected target group. The group itself is still declared by `TargetingDef`;
/// this input does not add a target role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramGroupInput {
    pub instruction_address: u32,
    pub group_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompiledInstructionKind {
    ImportCastPayment,
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
        departure_policy: ChooseDeparturePolicy,
        branches: Vec<CompiledChooseBranch>,
        otherwise: CompiledBlock,
    },
}

/// Compiled CR 800.4 departure behavior for one parked Program choice. Ordinary choices retain
/// the existing delegate flow; supported optional action-cost choices fall through unpaid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChooseDeparturePolicy {
    DelegateNonCostChoice,
    OptionalCostFallsThrough,
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
            ResolutionCost::SacrificePermanent { .. } => "Sacrifice a permanent".into(),
            ResolutionCost::TapPermanents { count, .. } => {
                format!("Tap {count} permanent(s)")
            }
            ResolutionCost::ExileGraveyardCard { .. } => "Exile a card from your graveyard".into(),
            ResolutionCost::DiscardCard {
                filter: Some(CardTypeFilter::Creature),
            } => "Discard a creature card".into(),
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
    /// The event-captured TriggerObject remains a legal linked-Exile subject under the existing
    /// object-generation and zone rules.
    TriggerObjectCanBeLinkedExiled,
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
    /// Non-targeting target-group operands keyed to their compiled instruction address.
    #[serde(default)]
    pub group_inputs: Vec<ProgramGroupInput>,
    pub body: CompiledBlock,
    pub slots: Vec<CompiledSlot>,
}

struct Compiler<'a> {
    owner: ProgramOwnerId,
    context: EffectContext,
    targeting: Option<&'a TargetingDef>,
    target_references: Vec<ProgramTargetReference>,
    cast_payment_profile: Option<ProgramCastPaymentProfile>,
    cast_cost_options: HashMap<ChoiceId, HashSet<ChoiceId>>,
    slots: Vec<CompiledSlot>,
    instruction_ids: HashSet<String>,
    choice_ids: HashSet<ChoiceId>,
    required_inputs: std::collections::BTreeSet<ProgramRequiredInput>,
    allow_trigger_object_linked_exile: bool,
    allow_mimic_vat_linked_exile_receipt: bool,
    allow_mosswort_linked_play: bool,
    allow_equipment_receipt: bool,
    allow_vow_equipment_choice: bool,
    allow_koya_outer_exile_result: bool,
    next_instruction_address: u32,
}

/// The only cast-payment import profile currently compiled for an authored Program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProgramCastPaymentProfile {
    MandatorySingleDiscard,
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
        Self::compile_with_targeting_cast_cost_groups_and_payment_profile(
            owner,
            context,
            program,
            targeting,
            cast_cost_groups,
            None,
        )
    }

    /// Compile an exact face owner with one closed, face-validated cast-payment import profile.
    pub(crate) fn compile_with_targeting_cast_cost_groups_and_payment_profile(
        owner: ProgramOwnerId,
        context: EffectContext,
        program: &CardProgram,
        targeting: Option<&TargetingDef>,
        cast_cost_groups: &[CastCostGroupDef],
        cast_payment_profile: Option<ProgramCastPaymentProfile>,
    ) -> Result<Self, String> {
        let mut compiled = Self::compile_internal(
            owner,
            context,
            program,
            targeting,
            cast_cost_groups,
            cast_payment_profile,
        )?;
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
                let role_index = authored_target_role_index(
                    effect,
                    reference.role,
                    is_vow_to_erebor_face(&compiled.owner),
                )
                .ok_or_else(|| {
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
        validate_targeted_player_damage_pairs(&compiled)?;
        let schema = TargetSchema::compile_program(&compiled, targeting)?;
        validate_spell_target_damage_history_profile(&compiled, &schema, targeting)?;
        validate_target_card_result_groups(&compiled, &schema)?;
        compiled.group_inputs = compile_group_inputs(&compiled, context, &schema)?;
        validate_terminal_linked_play_profile(&compiled)?;
        Ok(compiled)
    }

    fn compile_internal(
        owner: ProgramOwnerId,
        context: EffectContext,
        program: &CardProgram,
        targeting: Option<&TargetingDef>,
        cast_cost_groups: &[CastCostGroupDef],
        cast_payment_profile: Option<ProgramCastPaymentProfile>,
    ) -> Result<Self, String> {
        if owner.effect_context() != context {
            return Err("program owner does not match the supplied effect context".into());
        }
        if program.body.steps.is_empty() {
            return Err("program body must contain at least one instruction".into());
        }
        let target_projection = owner.target_projection();
        let allow_mosswort_linked_play = is_mosswort_bridge_activated_02(&owner);
        let mut compiler = Compiler {
            owner: owner.clone(),
            context,
            targeting,
            target_references: program.target_bindings.clone(),
            cast_payment_profile,
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
            allow_trigger_object_linked_exile: false,
            allow_mimic_vat_linked_exile_receipt: false,
            allow_mosswort_linked_play,
            allow_equipment_receipt: is_kyoshi_battle_fan_receipt_owner(&owner),
            allow_vow_equipment_choice: is_vow_to_erebor_face(&owner),
            allow_koya_outer_exile_result: is_koya_outer_program_owner(&owner),
            next_instruction_address: 0,
        };
        let (body, _) = compiler.compile_block(&program.body, &HashMap::new())?;
        Ok(Self {
            owner,
            context,
            target_projection,
            required_inputs: compiler.required_inputs.into_iter().collect(),
            target_bindings: Vec::new(),
            group_inputs: Vec::new(),
            body,
            slots: compiler.slots,
        })
    }

    pub fn slot(&self, id: ResultSlotId) -> Option<&CompiledSlot> {
        self.slots.get(id.0 as usize)
    }
}

fn spell_destroy_target_group(
    instruction_id: &str,
    references: &[ProgramTargetReference],
) -> Result<u32, String> {
    let reference = single_instruction_target_binding(
        instruction_id,
        references,
        ProgramTargetRole::Subject,
        "spell_program Destroy",
    )?;
    Ok(reference.target_group_index)
}

fn spell_hand_discard_target_group(
    instruction_id: &str,
    references: &[ProgramTargetReference],
) -> Result<u32, String> {
    let reference = single_instruction_target_binding(
        instruction_id,
        references,
        ProgramTargetRole::Target,
        "spell_program ChooseHandCards",
    )?;
    Ok(reference.target_group_index)
}

fn chosen_single_target_group(
    instruction_id: &str,
    references: &[ProgramTargetReference],
    targeting: Option<&TargetingDef>,
    role: ProgramTargetRole,
    operation: &str,
) -> Result<(u32, u32), String> {
    let reference = single_instruction_target_binding(instruction_id, references, role, operation)?;
    let targeting =
        targeting.ok_or_else(|| format!("{operation} requires an authored target group"))?;
    let group = targeting
        .groups
        .get(reference.target_group_index as usize)
        .ok_or_else(|| "program target binding references an unknown target group".to_string())?;
    if group.min != 1 || group.max != 1 {
        return Err(format!(
            "{operation} requires one required single-target group"
        ));
    }
    Ok((reference.target_group_index, group.max))
}

fn targeted_opponent_singleton_choice_group(effect: &SpellEffectKind) -> Option<u32> {
    let SpellEffectKind::ChoosePermanents {
        chooser:
            PlayerRecipient::TargetedPlayer {
                group_index,
                kind: TargetKind::OpponentPlayer,
            },
        filter,
        min: 1,
        max: 1,
        constraints,
    } = effect
    else {
        return None;
    };
    let expected_filter = TargetFilter {
        kind: TargetKind::AnyPermanent,
        controller: super::TargetController::You,
        ..TargetFilter::default()
    };
    (filter == &expected_filter && constraints.is_empty()).then_some(*group_index)
}

fn targeted_opponent_nonland_shuffle_group(effect: &SpellEffectKind) -> Option<u32> {
    match effect {
        SpellEffectKind::ShuffleNonlandPermanentsIntoOwnersLibraries {
            players:
                RelativePlayerSet::TargetedPlayer {
                    group_index,
                    kind: TargetKind::OpponentPlayer,
                },
        } => Some(*group_index),
        _ => None,
    }
}

fn single_source_chosen_shuffle_filter(subjects: &[EffectSubject]) -> Option<&TargetFilter> {
    match subjects {
        [EffectSubject::Source, EffectSubject::Chosen(filter)]
        | [EffectSubject::Chosen(filter), EffectSubject::Source] => Some(filter),
        _ => None,
    }
}

fn spell_exile_target_group(
    instruction_id: &str,
    references: &[ProgramTargetReference],
    targeting: Option<&TargetingDef>,
) -> Result<(u32, u32), String> {
    let reference = single_instruction_target_binding(
        instruction_id,
        references,
        ProgramTargetRole::Subject,
        "spell_program Exile",
    )?;
    let targeting = targeting
        .ok_or_else(|| "spell_program Exile requires an authored target group".to_string())?;
    let group = targeting
        .groups
        .get(reference.target_group_index as usize)
        .ok_or_else(|| "program target binding references an unknown target group".to_string())?;
    Ok((reference.target_group_index, group.max))
}

fn single_instruction_target_binding<'a>(
    instruction_id: &str,
    references: &'a [ProgramTargetReference],
    role: ProgramTargetRole,
    operation: &str,
) -> Result<&'a ProgramTargetReference, String> {
    let role_name = match role {
        ProgramTargetRole::Subject => "Subject",
        ProgramTargetRole::Target => "Target",
        ProgramTargetRole::StackAbility => "StackAbility",
        ProgramTargetRole::Spell => "Spell",
        ProgramTargetRole::Source => "Source",
    };
    let mut matches = references
        .iter()
        .filter(|reference| reference.instruction_id == instruction_id);
    let Some(reference) = matches.next() else {
        return Err(format!(
            "{operation} instruction '{instruction_id}' requires one explicit {role_name} target binding"
        ));
    };
    if reference.role != role || matches.next().is_some() {
        return Err(format!(
            "{operation} instruction '{instruction_id}' requires exactly one explicit {role_name} target binding"
        ));
    }
    Ok(reference)
}

fn is_spell_target_damage_history_condition(condition: &GameCondition) -> bool {
    matches!(
        condition,
        GameCondition::ObjectWasDealtDamageThisTurn {
            object: super::ConditionObjectRef::ChosenTarget {
                group_index: 0,
                target_index: 0,
            }
        }
    )
}

fn is_exact_creature_target_filter(filter: &TargetFilter) -> bool {
    filter
        == &TargetFilter {
            kind: TargetKind::Creature,
            ..TargetFilter::default()
        }
}

fn validate_spell_target_damage_history_profile(
    program: &CompiledProgram,
    schema: &TargetSchema<'_, '_>,
    targeting: Option<&TargetingDef>,
) -> Result<(), String> {
    let mut history_if_addresses = Vec::new();
    collect_spell_target_damage_history_if_addresses(&program.body, &mut history_if_addresses);
    if history_if_addresses.is_empty() {
        return Ok(());
    }

    let invalid_profile = || {
        "spell_program target damage history requires a prior Subject-bound Exile of the required singleton Creature group 0".to_string()
    };
    if program.context != EffectContext::Spell || !matches!(&program.owner, ProgramOwnerId::Face(_))
    {
        return Err(invalid_profile());
    }
    let Some(targeting) = targeting else {
        return Err(invalid_profile());
    };
    let ([target_group], [schema_group]) = (targeting.groups.as_slice(), schema.groups.as_slice())
    else {
        return Err(invalid_profile());
    };
    if target_group.chooser != super::TargetChooser::Controller
        || target_group.min != 1
        || target_group.max != 1
        || !target_group.effect_indices.is_empty()
        || !target_group.distinct_from.is_empty()
        || target_group.same_graveyard
        || target_group.cast_cost_expansion.is_some()
        || schema_group.chooser != super::TargetChooser::Controller
        || schema_group.min != 1
        || schema_group.max != 1
        || !matches!(
            schema_group.bindings.as_slice(),
            [binding]
                if matches!(
                    binding.role,
                    super::TargetRole::Filtered(filter)
                        if is_exact_creature_target_filter(filter)
                )
        )
    {
        return Err(invalid_profile());
    }

    let [target_binding] = program.target_bindings.as_slice() else {
        return Err(invalid_profile());
    };
    if target_binding.target_group_index != 0 || target_binding.role_index != 0 {
        return Err(invalid_profile());
    }
    let bound_exile = matches!(
        find_effect_by_address(&program.body, target_binding.instruction_address),
        Some(SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(filter),
            linked_exile_id: None,
        }) if is_exact_creature_target_filter(filter)
    );
    if !bound_exile
        || history_if_addresses
            .iter()
            .any(|address| *address <= target_binding.instruction_address)
    {
        return Err(invalid_profile());
    }
    Ok(())
}

fn collect_spell_target_damage_history_if_addresses(
    block: &CompiledBlock,
    addresses: &mut Vec<u32>,
) {
    for instruction in &block.steps {
        match &instruction.kind {
            CompiledInstructionKind::If { condition, yes, no } => {
                if matches!(
                    condition.as_ref(),
                    CompiledProgramCondition::Game(condition)
                        if is_spell_target_damage_history_condition(condition)
                ) {
                    addresses.push(instruction.instruction_address);
                }
                collect_spell_target_damage_history_if_addresses(yes, addresses);
                collect_spell_target_damage_history_if_addresses(no, addresses);
            }
            CompiledInstructionKind::Choose {
                branches,
                otherwise,
                ..
            } => {
                for branch in branches {
                    collect_spell_target_damage_history_if_addresses(&branch.body, addresses);
                }
                collect_spell_target_damage_history_if_addresses(otherwise, addresses);
            }
            CompiledInstructionKind::Perform(_) | CompiledInstructionKind::ImportCastPayment => {}
        }
    }
}

fn ability_graveyard_exile_target_group(
    instruction_id: &str,
    references: &[ProgramTargetReference],
    targeting: Option<&TargetingDef>,
) -> Result<(u32, u32), String> {
    let reference = single_instruction_target_binding(
        instruction_id,
        references,
        ProgramTargetRole::Target,
        "ability_program MoveGraveyardCards Exile",
    )?;
    let targeting = targeting.ok_or_else(|| {
        "ability_program graveyard Exile result requires an authored target group".to_string()
    })?;
    if targeting.groups.len() != 1 {
        return Err(
            "ability_program graveyard Exile result requires exactly one target group".into(),
        );
    }
    let target_group_index = reference.target_group_index;
    let group = targeting
        .groups
        .get(target_group_index as usize)
        .ok_or_else(|| "program target binding references an unknown target group".to_string())?;
    if group.chooser != super::TargetChooser::Controller
        || group.min != 0
        || group.max != 2
        || !group.same_graveyard
        || !group.effect_indices.is_empty()
        || !group.distinct_from.is_empty()
        || group.cast_cost_expansion.is_some()
    {
        return Err(
            "ability_program graveyard Exile result requires one controller-chosen 0..2 same-graveyard target group".into(),
        );
    }
    Ok((target_group_index, group.max))
}

fn counter_target_ability_target_group(
    instruction_id: &str,
    references: &[ProgramTargetReference],
    targeting: Option<&TargetingDef>,
) -> Result<(u32, u32), String> {
    let reference = single_instruction_target_binding(
        instruction_id,
        references,
        ProgramTargetRole::StackAbility,
        "CounterTargetAbility",
    )?;
    let targeting = targeting.ok_or_else(|| {
        "CounterTargetAbility requires an authored StackAbility target group".to_string()
    })?;
    let group = targeting
        .groups
        .get(reference.target_group_index as usize)
        .ok_or_else(|| "program target binding references an unknown target group".to_string())?;
    if !matches!(group.min, 0 | 1) || group.max != 1 {
        return Err("CounterTargetAbility target group must have bounds 0..1 or 1..1".into());
    }
    Ok((reference.target_group_index, group.max))
}

fn validate_target_card_result_groups(
    program: &CompiledProgram,
    schema: &TargetSchema<'_, '_>,
) -> Result<(), String> {
    for slot in &program.slots {
        let ResultType::TargetCardResultCohort {
            action,
            target_group_index,
            ..
        } = &slot.result_type
        else {
            continue;
        };
        let group = schema
            .groups
            .get(*target_group_index as usize)
            .ok_or_else(|| "targeted card result references an unknown target group".to_string())?;
        match action {
            CardResultAction::Discard | CardResultAction::Destroy
                if group.min == 1 && group.max == 1 => {}
            CardResultAction::Exile
                if program.context == EffectContext::Ability
                    && schema.groups.len() == 1
                    && group.chooser == super::TargetChooser::Controller
                    && group.min == 0
                    && group.max == 2
                    && group.same_graveyard
                    && group.bindings.len() == 1
                    && matches!(group.bindings[0].role, super::TargetRole::GraveyardCard(_)) => {}
            CardResultAction::Exile => {
                return Err(
                    "ability_program graveyard Exile result requires one controller-chosen 0..2 same-graveyard card target group".into(),
                );
            }
            action => {
                let label = match action {
                    CardResultAction::Discard => "ChooseHandCards discard",
                    CardResultAction::Destroy => "Destroy",
                    _ => "targeted card result",
                };
                return Err(format!(
                    "{label} result requires a target group whose bounds are exactly 1..1"
                ));
            }
        }
    }
    Ok(())
}

fn authored_target_role_index(
    effect: &SpellEffectKind,
    role: ProgramTargetRole,
    allow_vow_equipment_choice: bool,
) -> Option<usize> {
    match (effect, role) {
        (SpellEffectKind::DamageTarget { .. }, ProgramTargetRole::Target)
        | (
            SpellEffectKind::DamagePlayer {
                who:
                    PlayerRecipient::TargetedPlayer {
                        kind: TargetKind::OpponentPlayer,
                        ..
                    },
                ..
            },
            ProgramTargetRole::Target,
        )
        | (SpellEffectKind::CounterTargetSpell { .. }, ProgramTargetRole::Spell)
        | (SpellEffectKind::CounterTargetAbility, ProgramTargetRole::StackAbility)
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
            SpellEffectKind::ChoosePermanents {
                chooser:
                    PlayerRecipient::TargetedPlayer {
                        kind: TargetKind::OpponentPlayer,
                        ..
                    },
                ..
            },
            ProgramTargetRole::Target,
        )
        | (
            SpellEffectKind::ShuffleNonlandPermanentsIntoOwnersLibraries {
                players:
                    RelativePlayerSet::TargetedPlayer {
                        kind: TargetKind::OpponentPlayer,
                        ..
                    },
            },
            ProgramTargetRole::Target,
        )
        | (
            SpellEffectKind::PutInOwnersLibrary {
                subject: EffectSubject::Chosen(_),
                ..
            },
            ProgramTargetRole::Subject,
        )
        | (
            SpellEffectKind::PutCounters {
                subject: EffectSubject::Chosen(_),
                ..
            },
            ProgramTargetRole::Target,
        )
        | (
            SpellEffectKind::Tap {
                subject: EffectSubject::Chosen(_),
            },
            ProgramTargetRole::Target,
        )
        | (
            SpellEffectKind::ApplyCombatRestriction {
                scope: super::CombatRestrictionScope::Chosen(_),
                ..
            },
            ProgramTargetRole::Target,
        )
        | (
            SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(_),
            },
            ProgramTargetRole::Subject,
        )
        | (
            SpellEffectKind::Exile {
                subject: EffectSubject::Chosen(_),
                linked_exile_id: None,
            },
            ProgramTargetRole::Subject,
        )
        | (
            SpellEffectKind::MoveGraveyardCards {
                destination: GraveyardDestination::Exile,
                ..
            },
            ProgramTargetRole::Target,
        )
        | (
            SpellEffectKind::ChooseCohortPartition {
                chooser: PlayerRecipient::TargetedPlayer { .. },
                ..
            },
            ProgramTargetRole::Target,
        )
        | (SpellEffectKind::ChooseHandCards { .. }, ProgramTargetRole::Target) => Some(0),
        (
            SpellEffectKind::ShufflePermanentsIntoOwnersLibraries { subjects },
            ProgramTargetRole::Target,
        ) if single_source_chosen_shuffle_filter(subjects).is_some() => Some(0),
        _ if allow_vow_equipment_choice
            && role == ProgramTargetRole::Target
            && is_vow_chosen_target_effect(effect) =>
        {
            Some(0)
        }
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
            CompiledInstructionKind::Perform(_) | CompiledInstructionKind::ImportCastPayment => {}
        }
    }
    None
}

#[derive(Debug, Clone, Copy)]
struct TargetedPlayerProgramUse {
    instruction_address: u32,
    group_index: u32,
    kind: TargetKind,
    is_damage: bool,
}

fn collect_targeted_player_program_uses(
    block: &CompiledBlock,
    uses: &mut Vec<TargetedPlayerProgramUse>,
) {
    for instruction in &block.steps {
        match &instruction.kind {
            CompiledInstructionKind::ImportCastPayment => {}
            CompiledInstructionKind::Perform(effect) => {
                if let SpellEffectKind::DamagePlayer {
                    who: PlayerRecipient::TargetedPlayer { group_index, kind },
                    ..
                } = effect.as_ref()
                {
                    uses.push(TargetedPlayerProgramUse {
                        instruction_address: instruction.instruction_address,
                        group_index: *group_index,
                        kind: *kind,
                        is_damage: true,
                    });
                }
            }
            CompiledInstructionKind::If { yes, no, .. } => {
                collect_targeted_player_program_uses(yes, uses);
                collect_targeted_player_program_uses(no, uses);
            }
            CompiledInstructionKind::Choose {
                chooser,
                branches,
                otherwise,
                ..
            } => {
                if let PlayerRecipient::TargetedPlayer { group_index, kind } = chooser {
                    uses.push(TargetedPlayerProgramUse {
                        instruction_address: instruction.instruction_address,
                        group_index: *group_index,
                        kind: *kind,
                        is_damage: false,
                    });
                }
                for branch in branches {
                    collect_targeted_player_program_uses(&branch.body, uses);
                }
                collect_targeted_player_program_uses(otherwise, uses);
            }
        }
    }
}

fn validate_targeted_player_damage_pairs(program: &CompiledProgram) -> Result<(), String> {
    let mut uses = Vec::new();
    collect_targeted_player_program_uses(&program.body, &mut uses);
    for damage in uses.iter().filter(|usage| usage.is_damage) {
        if damage.kind != TargetKind::OpponentPlayer {
            return Err(format!(
                "DamagePlayer instruction at address {} supports only an OpponentPlayer target chooser",
                damage.instruction_address
            ));
        }
        let chooser_count = uses
            .iter()
            .filter(|usage| {
                !usage.is_damage
                    && usage.group_index == damage.group_index
                    && usage.kind == damage.kind
            })
            .count();
        if chooser_count != 1 {
            return Err(format!(
                "DamagePlayer instruction at address {} requires exactly one matching TargetedPlayer Choose instruction for group {}",
                damage.instruction_address, damage.group_index
            ));
        }
        let damage_bindings = program
            .target_bindings
            .iter()
            .filter(|binding| binding.instruction_address == damage.instruction_address)
            .collect::<Vec<_>>();
        if damage_bindings.len() != 1
            || damage_bindings[0].target_group_index != damage.group_index
            || damage_bindings[0].role_index != 0
        {
            return Err(format!(
                "DamagePlayer instruction at address {} requires exactly one Target role binding to its own OpponentPlayer target group {}",
                damage.instruction_address, damage.group_index
            ));
        }
    }
    for chooser in uses.iter().filter(|usage| !usage.is_damage) {
        if chooser.kind != TargetKind::OpponentPlayer {
            return Err(format!(
                "TargetedPlayer Choose instruction at address {} supports only OpponentPlayer targets",
                chooser.instruction_address
            ));
        }
        let damage_count = uses
            .iter()
            .filter(|usage| {
                usage.is_damage
                    && usage.group_index == chooser.group_index
                    && usage.kind == chooser.kind
            })
            .count();
        if damage_count != 1 {
            return Err(format!(
                "TargetedPlayer Choose instruction at address {} requires exactly one matching bound DamagePlayer target for group {}",
                chooser.instruction_address, chooser.group_index
            ));
        }
    }
    Ok(())
}

fn find_effect_by_address(block: &CompiledBlock, address: u32) -> Option<&SpellEffectKind> {
    for instruction in &block.steps {
        if instruction.instruction_address == address {
            if let CompiledInstructionKind::Perform(effect) = &instruction.kind {
                return Some(effect.as_ref());
            }
            return None;
        }
        let nested =
            match &instruction.kind {
                CompiledInstructionKind::If { yes, no, .. } => find_effect_by_address(yes, address)
                    .or_else(|| find_effect_by_address(no, address)),
                CompiledInstructionKind::Choose {
                    branches,
                    otherwise,
                    ..
                } => branches
                    .iter()
                    .find_map(|branch| find_effect_by_address(&branch.body, address))
                    .or_else(|| find_effect_by_address(otherwise, address)),
                CompiledInstructionKind::Perform(_)
                | CompiledInstructionKind::ImportCastPayment => None,
            };
        if nested.is_some() {
            return nested;
        }
    }
    None
}

fn collect_group_inputs(block: &CompiledBlock, inputs: &mut Vec<ProgramGroupInput>) {
    for instruction in &block.steps {
        match &instruction.kind {
            CompiledInstructionKind::ImportCastPayment => {}
            CompiledInstructionKind::Perform(effect) => {
                if let SpellEffectKind::DamagePlayer {
                    who: PlayerRecipient::ControllerOfTargetGroup { group_index },
                    ..
                } = effect.as_ref()
                {
                    inputs.push(ProgramGroupInput {
                        instruction_address: instruction.instruction_address,
                        group_index: *group_index,
                    });
                }
            }
            CompiledInstructionKind::If { yes, no, .. } => {
                collect_group_inputs(yes, inputs);
                collect_group_inputs(no, inputs);
            }
            CompiledInstructionKind::Choose {
                branches,
                otherwise,
                ..
            } => {
                for branch in branches {
                    collect_group_inputs(&branch.body, inputs);
                }
                collect_group_inputs(otherwise, inputs);
            }
        }
    }
}

fn compile_group_inputs(
    program: &CompiledProgram,
    context: EffectContext,
    schema: &TargetSchema<'_, '_>,
) -> Result<Vec<ProgramGroupInput>, String> {
    let mut inputs = Vec::new();
    collect_group_inputs(&program.body, &mut inputs);
    for input in &inputs {
        if context != EffectContext::Spell {
            return Err(
                "DamagePlayer ControllerOfTargetGroup in spell_program requires a spell owner"
                    .into(),
            );
        }
        let group = schema
            .groups
            .get(input.group_index as usize)
            .ok_or_else(|| {
                "DamagePlayer recipient references an unknown target group".to_string()
            })?;
        if group.min != 1 || group.max != 1 || group.bindings.len() != 1 {
            return Err(
                "DamagePlayer ControllerOfTargetGroup requires one target in a singleton group bound to its DamageTarget role".into(),
            );
        }
        let binding = group.bindings[0];
        let target_address = u32::try_from(binding.effect_index)
            .map_err(|_| "target instruction address exceeds u32".to_string())?;
        let owns_target_role = program.target_bindings.iter().any(|target_binding| {
            target_binding.instruction_address == target_address
                && target_binding.target_group_index == input.group_index
                && target_binding.role_index as usize == binding.role_index
        });
        let genuine_creature_damage_target = owns_target_role
            && matches!(
                (binding.role, find_effect_by_address(&program.body, target_address)),
                (
                    super::TargetRole::Filtered(filter),
                    Some(SpellEffectKind::DamageTarget { target, .. })
                ) if filter.kind == TargetKind::Creature && target.kind == TargetKind::Creature
            );
        if !genuine_creature_damage_target {
            return Err(
                "DamagePlayer ControllerOfTargetGroup requires the singleton group to bind a Creature DamageTarget role".into(),
            );
        }
    }
    Ok(inputs)
}

impl Compiler<'_> {
    fn compile_block(
        &mut self,
        authored: &AuthoredBlock,
        inherited: &HashMap<String, ResultSlotId>,
    ) -> Result<(CompiledBlock, Vec<ResultType>), String> {
        let mut names = inherited.clone();
        let mut steps: Vec<CompiledInstruction> = Vec::with_capacity(authored.steps.len());
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
                InstructionKind::ImportCastPayment => {
                    if instruction.input.is_some() {
                        return Err(format!(
                            "ImportCastPayment instruction '{}' cannot consume an input result",
                            instruction.id
                        ));
                    }
                    if instruction.bind.len() != 1 {
                        return Err(format!(
                            "ImportCastPayment instruction '{}' must bind its typed cohort result",
                            instruction.id
                        ));
                    }
                    if self.context != EffectContext::Spell
                        || self.cast_payment_profile
                            != Some(ProgramCastPaymentProfile::MandatorySingleDiscard)
                    {
                        return Err(
                            "ImportCastPayment requires a validated mandatory single-discard spell cost profile"
                                .into(),
                        );
                    }
                    (
                        None,
                        vec![ResultType::CastPaymentCohort {
                            action: CardResultAction::Discard,
                            cardinality: Cardinality {
                                min: 1,
                                max: Some(1),
                            },
                        }],
                        CompiledInstructionKind::ImportCastPayment,
                    )
                }
                InstructionKind::Perform(effect) => {
                    let input = compile_input(
                        effect,
                        instruction.input.as_deref(),
                        &names,
                        &self.slots,
                        self.allow_koya_outer_exile_result,
                    )?;
                    validate_program_gain_life_result_count_input(
                        effect,
                        input,
                        steps.last(),
                        &self.slots,
                    )?;
                    validate_previous_mill_mana_value_sum_input(
                        effect,
                        input,
                        steps.last(),
                        &self.slots,
                    )?;
                    validate_koya_exile_delayed_input(
                        effect,
                        input,
                        steps.last(),
                        &self.slots,
                        self.allow_koya_outer_exile_result,
                    )?;
                    validate_return_to_owners_hand_input(effect, input, &self.slots)?;
                    if operation_needs_affected_player(effect) {
                        self.required_inputs
                            .insert(ProgramRequiredInput::TriggerAffectedPlayer);
                    }
                    let mut target_group_max = None;
                    let target_group_index = if matches!(
                        effect.as_ref(),
                        SpellEffectKind::CounterTargetAbility
                    ) {
                        let (group_index, group_max) = counter_target_ability_target_group(
                            &instruction.id,
                            &self.target_references,
                            self.targeting,
                        )?;
                        target_group_max = Some(group_max);
                        Some(group_index)
                    } else if self.context == EffectContext::Spell {
                        match effect.as_ref() {
                            SpellEffectKind::Destroy {
                                subject: EffectSubject::Chosen(_),
                            } => Some(spell_destroy_target_group(
                                &instruction.id,
                                &self.target_references,
                            )?),
                            SpellEffectKind::Exile {
                                subject: EffectSubject::Chosen(_),
                                linked_exile_id: None,
                            } => {
                                let (group_index, group_max) = spell_exile_target_group(
                                    &instruction.id,
                                    &self.target_references,
                                    self.targeting,
                                )?;
                                target_group_max = Some(group_max);
                                Some(group_index)
                            }
                            SpellEffectKind::ChooseHandCards { .. } => {
                                Some(spell_hand_discard_target_group(
                                    &instruction.id,
                                    &self.target_references,
                                )?)
                            }
                            SpellEffectKind::PutInOwnersLibrary {
                                subject: EffectSubject::Chosen(_),
                                ..
                            } => {
                                let (group_index, group_max) = chosen_single_target_group(
                                    &instruction.id,
                                    &self.target_references,
                                    self.targeting,
                                    ProgramTargetRole::Subject,
                                    "PutInOwnersLibrary",
                                )?;
                                target_group_max = Some(group_max);
                                Some(group_index)
                            }
                            SpellEffectKind::PutCounters {
                                subject: EffectSubject::Chosen(_),
                                ..
                            } => {
                                let (group_index, group_max) = chosen_single_target_group(
                                    &instruction.id,
                                    &self.target_references,
                                    self.targeting,
                                    ProgramTargetRole::Target,
                                    "PutCounters",
                                )?;
                                target_group_max = Some(group_max);
                                Some(group_index)
                            }
                            SpellEffectKind::ApplyCombatRestriction {
                                scope: super::CombatRestrictionScope::Chosen(_),
                                ..
                            } => {
                                let (group_index, group_max) = chosen_single_target_group(
                                    &instruction.id,
                                    &self.target_references,
                                    self.targeting,
                                    ProgramTargetRole::Target,
                                    "ApplyCombatRestriction",
                                )?;
                                target_group_max = Some(group_max);
                                Some(group_index)
                            }
                            _ => None,
                        }
                    } else if self.context == EffectContext::Ability {
                        match effect.as_ref() {
                            SpellEffectKind::Tap {
                                subject: EffectSubject::Chosen(_),
                            }
                            | SpellEffectKind::PutCounters {
                                counter: super::CounterKind::Stun,
                                count: Amount::Fixed(1),
                                subject: EffectSubject::Chosen(_),
                            } => {
                                let (group_index, group_max) = chosen_single_target_group(
                                    &instruction.id,
                                    &self.target_references,
                                    self.targeting,
                                    ProgramTargetRole::Target,
                                    "chosen permanent effect",
                                )?;
                                target_group_max = Some(group_max);
                                Some(group_index)
                            }
                            SpellEffectKind::ShufflePermanentsIntoOwnersLibraries { subjects }
                                if single_source_chosen_shuffle_filter(subjects).is_some() =>
                            {
                                let (group_index, group_max) = chosen_single_target_group(
                                    &instruction.id,
                                    &self.target_references,
                                    self.targeting,
                                    ProgramTargetRole::Target,
                                    "ShufflePermanentsIntoOwnersLibraries",
                                )?;
                                target_group_max = Some(group_max);
                                Some(group_index)
                            }
                            SpellEffectKind::ChoosePermanents { .. }
                                if targeted_opponent_singleton_choice_group(effect).is_some() =>
                            {
                                let expected_group =
                                    targeted_opponent_singleton_choice_group(effect)
                                        .expect("guarded above");
                                let (group_index, group_max) = chosen_single_target_group(
                                    &instruction.id,
                                    &self.target_references,
                                    self.targeting,
                                    ProgramTargetRole::Target,
                                    "ChoosePermanents targeted-opponent choice",
                                )?;
                                if group_index != expected_group {
                                    return Err("ChoosePermanents targeted-opponent choice must use its chooser target group".into());
                                }
                                target_group_max = Some(group_max);
                                Some(group_index)
                            }
                            SpellEffectKind::ShuffleNonlandPermanentsIntoOwnersLibraries {
                                ..
                            } if targeted_opponent_nonland_shuffle_group(effect).is_some() => {
                                let expected_group =
                                    targeted_opponent_nonland_shuffle_group(effect)
                                        .expect("guarded above");
                                let (group_index, group_max) = chosen_single_target_group(
                                    &instruction.id,
                                    &self.target_references,
                                    self.targeting,
                                    ProgramTargetRole::Target,
                                    "ShuffleNonlandPermanentsIntoOwnersLibraries",
                                )?;
                                if group_index != expected_group {
                                    return Err("ShuffleNonlandPermanentsIntoOwnersLibraries must use its targeted-player group".into());
                                }
                                target_group_max = Some(group_max);
                                Some(group_index)
                            }
                            SpellEffectKind::Exile {
                                subject: EffectSubject::Chosen(_),
                                linked_exile_id: None,
                            } if self.allow_koya_outer_exile_result => {
                                let (group_index, group_max) = koya_outer_exile_target_group(
                                    &instruction.id,
                                    &self.target_references,
                                    self.targeting,
                                )?;
                                target_group_max = Some(group_max);
                                Some(group_index)
                            }
                            SpellEffectKind::MoveGraveyardCards {
                                destination: GraveyardDestination::Exile,
                                ..
                            } => {
                                let (group_index, group_max) =
                                    ability_graveyard_exile_target_group(
                                        &instruction.id,
                                        &self.target_references,
                                        self.targeting,
                                    )?;
                                target_group_max = Some(group_max);
                                Some(group_index)
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if self.allow_vow_equipment_choice
                        && is_vow_chosen_target_effect(effect)
                        && !self.target_references.iter().any(|reference| {
                            reference.instruction_id == instruction.id
                                && reference.role == ProgramTargetRole::Target
                                && reference.target_group_index == 0
                        })
                    {
                        return Err(format!(
                            "Vow to Erebor instruction '{}' must bind its target to group 0",
                            instruction.id
                        ));
                    }
                    let result_types = validate_and_result_types(
                        effect,
                        self.context,
                        target_group_index,
                        target_group_max,
                        self,
                    )?;
                    validate_equipment_selection_input(
                        effect,
                        input,
                        steps.last(),
                        &self.slots,
                        self.allow_equipment_receipt,
                        self.allow_vow_equipment_choice,
                    )?;
                    if self.allow_trigger_object_linked_exile
                        && matches!(
                            effect.as_ref(),
                            SpellEffectKind::Exile {
                                subject: EffectSubject::TriggerObject,
                                linked_exile_id: Some(_),
                                ..
                            }
                        )
                    {
                        self.required_inputs
                            .insert(ProgramRequiredInput::TriggerLinkedExileObject);
                    }
                    if target_group_index.is_some()
                        && matches!(
                            effect.as_ref(),
                            SpellEffectKind::Destroy {
                                subject: EffectSubject::Chosen(_),
                            } | SpellEffectKind::ChooseHandCards { .. }
                        )
                        && (instruction.bind.len() != 1 || result_types.len() != 1)
                    {
                        let label =
                            if matches!(effect.as_ref(), SpellEffectKind::ChooseHandCards { .. }) {
                                "ChooseHandCards"
                            } else {
                                "Destroy"
                            };
                        return Err(format!(
                            "spell_program {label} instruction '{}' must bind its single typed result",
                            instruction.id
                        ));
                    }
                    if target_group_index.is_some()
                        && matches!(
                            effect.as_ref(),
                            SpellEffectKind::MoveGraveyardCards {
                                destination: GraveyardDestination::Exile,
                                ..
                            }
                        )
                        && (instruction.bind.len() != 1 || result_types.len() != 1)
                    {
                        return Err(format!(
                            "ability_program MoveGraveyardCards Exile instruction '{}' must bind its single typed result",
                            instruction.id
                        ));
                    }
                    if matches!(
                        effect.as_ref(),
                        SpellEffectKind::CounterTriggeringStackObjectUnlessPays { .. }
                    ) {
                        self.required_inputs
                            .insert(ProgramRequiredInput::TriggerAffectedPlayer);
                        self.required_inputs
                            .insert(ProgramRequiredInput::TriggerTargetingStackObject);
                    }
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
                            && !is_resultless_conditional_top_search(effect)
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
                    // Keep this family ability-owned and coherent: only the source itself may be
                    // inspected, and composites cannot mix source facts with unrelated contexts.
                    let condition = if self.context == EffectContext::Ability
                        && is_ability_source_condition_family(condition)
                    {
                        condition.validate_live().map_err(|reason| {
                            format!("If instruction '{}': {reason}", instruction.id)
                        })?;
                        CompiledProgramCondition::Game(condition.as_ref().clone())
                    } else if matches!(condition.as_ref(), GameCondition::Void) {
                        condition.validate_live().map_err(|reason| {
                            format!("If instruction '{}': {reason}", instruction.id)
                        })?;
                        CompiledProgramCondition::Game(condition.as_ref().clone())
                    } else if self.context == EffectContext::Spell
                        && is_exact_delirium_program_condition(condition)
                    {
                        // Preserve Impossible Inferno's existing Delirium check without
                        // admitting other DistinctCardTypes profiles to authored Programs.
                        condition.validate_live().map_err(|reason| {
                            format!("If instruction '{}': {reason}", instruction.id)
                        })?;
                        CompiledProgramCondition::Game(condition.as_ref().clone())
                    } else if self.context == EffectContext::Spell
                        && matches!(&self.owner, ProgramOwnerId::Face(_))
                        && is_spell_target_damage_history_condition(condition)
                    {
                        // Only a spell face uses the flattened stack target record read by this
                        // history query; modal mode targets are stored in chosen_modes instead.
                        // The final compiled face schema binds this to one prior Subject-bound
                        // Creature Exile; unsupported target/history shapes remain rejected.
                        condition.validate_live().map_err(|reason| {
                            format!("If instruction '{}': {reason}", instruction.id)
                        })?;
                        CompiledProgramCondition::Game(condition.as_ref().clone())
                    } else if self.context == EffectContext::Spell
                        && matches!(&self.owner, ProgramOwnerId::Face(_))
                        && matches!(condition.as_ref(), GameCondition::CastSnapshot { .. })
                    {
                        // A non-modal spell face may branch directly on a cast condition saved
                        // when it was cast. Keep this owner-specific exception out of the shared
                        // Program condition validator and Choose requirements.
                        condition.validate().map_err(|reason| {
                            format!("If instruction '{}': {reason}", instruction.id)
                        })?;
                        CompiledProgramCondition::Game(condition.as_ref().clone())
                    } else {
                        compile_program_condition(
                            &ProgramCondition::Game(condition.as_ref().clone()),
                            &names,
                            &self.slots,
                        )
                        .map_err(|reason| {
                            format!("If instruction '{}': {reason}", instruction.id)
                        })?
                    };
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
                    let targeted_opponent_chooser = matches!(
                        chooser,
                        PlayerRecipient::TargetedPlayer {
                            kind: TargetKind::OpponentPlayer,
                            ..
                        } if *optional
                            && matches!(self.context, EffectContext::Spell | EffectContext::Ability)
                    );
                    let chooser_supported = *chooser == PlayerRecipient::Controller
                        || targeted_opponent_chooser
                        || (matches!(
                            chooser,
                            PlayerRecipient::SourceController | PlayerRecipient::AffectedPlayer
                        ) && self.context == EffectContext::Ability);
                    if !chooser_supported {
                        return Err(format!(
                            "Choose instruction '{}' supports Controller, SourceController/AffectedPlayer on an ability program, or an optional OpponentPlayer target on a spell or ability program, as chooser",
                            instruction.id
                        ));
                    }
                    if *chooser == PlayerRecipient::AffectedPlayer {
                        self.required_inputs
                            .insert(ProgramRequiredInput::TriggerAffectedPlayer);
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
                    let mut optional_cost_falls_through = false;
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
                        if targeted_opponent_chooser
                            && !matches!(&branch.cost, ResolutionCost::None)
                        {
                            return Err(format!(
                                "Choose instruction '{}' branch '{}' only supports a free cost for an optional OpponentPlayer target chooser",
                                instruction.id, branch.branch_id
                            ));
                        }
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
                            ResolutionCost::DiscardCard {
                                filter: Some(CardTypeFilter::Creature),
                            } if *optional
                                && *chooser == PlayerRecipient::Controller
                                && self.context == EffectContext::Ability =>
                            {
                                has_paid_branch = true;
                            }
                            ResolutionCost::PutHandCardOnLibraryBottom
                                if *chooser == PlayerRecipient::Controller
                                    && self.context == EffectContext::Spell =>
                            {
                                has_paid_branch = true;
                            }
                            ResolutionCost::Blight { count }
                                if *chooser == PlayerRecipient::Controller
                                    && self.context == EffectContext::Ability
                                    && *count > 0 =>
                            {
                                has_paid_branch = true;
                            }
                            ResolutionCost::SacrificePermanent {
                                filter,
                                source_only,
                            } if *chooser == PlayerRecipient::Controller
                                && self.context == EffectContext::Ability =>
                            {
                                if program_sacrifice_has_any_of_branch_exclusions(filter) {
                                    return Err(format!(
                                        "Choose instruction '{}' branch '{}' sacrifice cost uses excluded_objects inside any_of, which typed Program does not support",
                                        instruction.id, branch.branch_id
                                    ));
                                }
                                super::effects::validate_resolution_sacrifice_cost(
                                    filter,
                                    *source_only,
                                )
                                .map_err(|reason| {
                                    format!(
                                        "Choose instruction '{}' branch '{}' sacrifice cost: {reason}",
                                        instruction.id, branch.branch_id
                                    )
                                })?;
                                has_paid_branch = true;
                            }
                            ResolutionCost::TapPermanents {
                                count: 1, filter, ..
                            } if *chooser == PlayerRecipient::Controller
                                && self.context == EffectContext::Ability =>
                            {
                                validate_program_tap_permanent_cost_filter(filter).map_err(
                                    |reason| {
                                        format!(
                                            "Choose instruction '{}' branch '{}' tap cost: {reason}",
                                            instruction.id, branch.branch_id
                                        )
                                    },
                                )?;
                                has_paid_branch = true;
                            }
                            ResolutionCost::ExileGraveyardCard { .. } if !*optional => {
                                return Err(format!(
                                    "Choose instruction '{}' graveyard-exile cost requires an optional Choose",
                                    instruction.id
                                ));
                            }
                            ResolutionCost::ExileGraveyardCard { filter }
                                if *optional
                                    && *chooser == PlayerRecipient::Controller
                                    && self.context == EffectContext::Spell =>
                            {
                                filter.as_ref().map_or(Ok(()), ZoneCardFilter::validate).map_err(
                                    |reason| {
                                        format!(
                                            "Choose instruction '{}' graveyard-exile cost: {reason}",
                                            instruction.id
                                        )
                                    },
                                )?;
                                has_paid_branch = true;
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
                                let compiled = if self.context == EffectContext::Spell
                                    && is_vow_to_erebor_face(&self.owner)
                                    && is_exact_vow_dwarf_target_guard(condition)
                                {
                                    condition.validate_live().map_err(|reason| {
                                        format!(
                                            "Choose instruction '{}' branch '{}': {reason}",
                                            instruction.id, branch.branch_id
                                        )
                                    })?;
                                    CompiledProgramCondition::Game(condition.clone())
                                } else if self.context == EffectContext::Ability
                                    && (is_ability_source_condition_family(condition)
                                        || is_exact_ability_choose_countered_creature_count(
                                            condition,
                                        ))
                                {
                                    condition.validate_live().map_err(|reason| {
                                        format!(
                                            "Choose instruction '{}' branch '{}': {reason}",
                                            instruction.id, branch.branch_id
                                        )
                                    })?;
                                    CompiledProgramCondition::Game(condition.clone())
                                } else {
                                    compile_program_condition(
                                        &ProgramCondition::Game(condition.clone()),
                                        &names,
                                        &self.slots,
                                    )
                                    .map_err(|reason| {
                                        format!(
                                            "Choose instruction '{}' branch '{}': {reason}",
                                            instruction.id, branch.branch_id
                                        )
                                    })?
                                };
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
                                let mimic_vat_receipt_body =
                                    exact_mimic_vat_imprint_receipt_body(&self.owner, &branch.body);
                                let exact_linked_exile_body =
                                    exact_trigger_object_linked_exile_body(&branch.body)
                                        || mimic_vat_receipt_body;
                                let exact_linked_exile_profile = exact_linked_exile_body
                                    && *chooser == PlayerRecipient::Controller
                                    && *optional
                                    && branches.len() == 1
                                    && branch.cost == ResolutionCost::None
                                    && otherwise.steps.is_empty()
                                    && otherwise.yields.is_empty();
                                if self.context != EffectContext::Ability
                                    || (!exact_source_plus_one_counter_body(&branch.body)
                                        && !exact_linked_exile_profile)
                                {
                                    return Err(format!(
                                        "Choose instruction '{}' supports EffectsApplicable only for an ability branch containing one resultless fixed positive Source +1/+1 PutCounters instruction, the exact optional Controller linked-Exile TriggerObject branch, or the exact Mimic Vat imprint linked-exile receipt flow",
                                        instruction.id
                                    ));
                                }
                                if exact_linked_exile_profile && mimic_vat_receipt_body {
                                    optional_cost_falls_through = true;
                                }
                                if exact_linked_exile_profile {
                                    CompiledChooseRequirement::TriggerObjectCanBeLinkedExiled
                                } else {
                                    CompiledChooseRequirement::SourceCanReceivePlusOnePlusOneCounters
                                }
                            }
                        };

                        let previous_linked_exile_permission =
                            self.allow_trigger_object_linked_exile;
                        let previous_mimic_receipt_permission =
                            self.allow_mimic_vat_linked_exile_receipt;
                        self.allow_trigger_object_linked_exile = matches!(
                            &requirement,
                            CompiledChooseRequirement::TriggerObjectCanBeLinkedExiled
                        );
                        self.allow_mimic_vat_linked_exile_receipt =
                            exact_mimic_vat_imprint_receipt_body(&self.owner, &branch.body)
                                && matches!(
                                    &requirement,
                                    CompiledChooseRequirement::TriggerObjectCanBeLinkedExiled
                                );
                        let compiled_body = self.compile_block(&branch.body, &names);
                        self.allow_trigger_object_linked_exile = previous_linked_exile_permission;
                        self.allow_mimic_vat_linked_exile_receipt =
                            previous_mimic_receipt_permission;
                        let (body, body_types) = compiled_body?;
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
                            departure_policy: if optional_cost_falls_through {
                                ChooseDeparturePolicy::OptionalCostFallsThrough
                            } else {
                                ChooseDeparturePolicy::DelegateNonCostChoice
                            },
                            branches: compiled_branches,
                            otherwise,
                        },
                    )
                }
            };
            if matches!(
                &instruction.kind,
                InstructionKind::Perform(effect)
                    if is_source_sacrifice_reflexive_consumer(effect)
            ) {
                let input_slot = input.ok_or_else(|| {
                    format!(
                        "CreateReflexiveTrigger instruction '{}' requires the immediately preceding Source sacrifice result",
                        instruction.id
                    )
                })?;
                let immediately_preceding_source_sacrifice = steps.last().is_some_and(|previous| {
                    previous.input.is_none()
                        && previous.outputs.len() == 1
                        && previous.outputs[0] == input_slot
                        && matches!(
                            &previous.kind,
                            CompiledInstructionKind::Perform(effect)
                                if matches!(
                                    effect.as_ref(),
                                    SpellEffectKind::Sacrifice {
                                        subject: EffectSubject::Source,
                                    }
                                )
                        )
                });
                if !immediately_preceding_source_sacrifice {
                    return Err(format!(
                        "CreateReflexiveTrigger instruction '{}' must immediately consume the direct Source sacrifice result in the same block",
                        instruction.id
                    ));
                }
            }
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
            who: PlayerRecipient::AffectedPlayer
                | PlayerRecipient::EachOtherPlayerThanAffectedPlayer,
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
        ProgramCondition::Result(ProgramResultCondition::CardResultCount {
            result,
            card_type,
            min,
            max,
        }) => {
            if min.is_none() && max.is_none() {
                return Err("card result count requires a minimum or maximum".into());
            }
            if min
                .zip(*max)
                .is_some_and(|(minimum, maximum)| minimum > maximum)
            {
                return Err("card result count minimum cannot exceed its maximum".into());
            }
            let slot = result_slot(result, names, slots)?;
            let cardinality = match &slots[slot.0 as usize].result_type {
                ResultType::CardResultCohort { cardinality, .. }
                | ResultType::TargetCardResultCohort { cardinality, .. }
                | ResultType::CastPaymentCohort { cardinality, .. }
                | ResultType::LinkedExileCardResultCohort { cardinality, .. } => cardinality,
                _ => {
                    return Err(format!(
                        "result '{result}' is not a compatible card-result cohort"
                    ));
                }
            };
            if cardinality.max.is_some_and(|upper| {
                min.is_some_and(|minimum| minimum > upper) || max.is_some_and(|bound| bound > upper)
            }) {
                return Err(format!(
                    "card result '{result}' predicate exceeds its typed cohort cardinality"
                ));
            }
            Ok(CompiledProgramCondition::Result(
                CompiledProgramResultCondition::CardResultCount {
                    result: slot,
                    card_type: *card_type,
                    min: *min,
                    max: *max,
                },
            ))
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
            let controller_count_from_each_player = *who == PlayerRecipient::EachPlayer
                && *players == RelativePlayerSet::Controller
                && card_type.is_none()
                && *min == Some(1)
                && max.is_none();
            if (*who != PlayerRecipient::Controller || *players != RelativePlayerSet::Controller)
                && !controller_count_from_each_player
            {
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
        ProgramCondition::Result(ProgramResultCondition::CounteredAbilitySourceMatches {
            result,
        }) => {
            let slot = result_slot(result, names, slots)?;
            if !matches!(
                &slots[slot.0 as usize].result_type,
                ResultType::CounteredAbilitySource {
                    cardinality: Cardinality {
                        min: 0,
                        max: Some(1),
                    },
                }
            ) {
                return Err(format!(
                    "result '{result}' is not a compatible CounteredAbilitySource"
                ));
            }
            Ok(CompiledProgramCondition::Result(
                CompiledProgramResultCondition::CounteredAbilitySourceMatches { result: slot },
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

fn is_source_sacrifice_reflexive_consumer(effect: &SpellEffectKind) -> bool {
    matches!(
        effect,
        SpellEffectKind::CreateReflexiveTrigger {
            when: Some(super::ResolutionReceiptCondition::Sacrificed {
                object: super::ConditionObjectRef::Source,
            }),
            ..
        }
    )
}

fn is_targeted_opponent_previous_mill_damage(effect: &EffectOp) -> bool {
    matches!(
        effect,
        EffectOp::DamagePlayer {
            amount: Amount::Count(super::CountExpression::PreviousMillManaValueSum),
            who: PlayerRecipient::TargetedPlayer {
                kind: TargetKind::OpponentPlayer,
                ..
            },
        }
    )
}

fn program_gain_life_result_count_profile(amount: &Amount) -> Option<(&CardResultFilter, u32)> {
    match amount {
        Amount::Count(CountExpression::CardsMatchingResult { filter }) => Some((filter, 1)),
        Amount::Count(CountExpression::Affine { constant: 0, terms }) => match terms.as_slice() {
            [QuantityTerm {
                coefficient,
                quantity: CountExpression::CardsMatchingResult { filter },
            }] if *coefficient > 0 => Some((filter, u32::try_from(*coefficient).ok()?)),
            _ => None,
        },
        _ => None,
    }
}

fn validate_program_gain_life_result_count_input(
    effect: &EffectOp,
    input: Option<ResultSlotId>,
    previous: Option<&CompiledInstruction>,
    slots: &[CompiledSlot],
) -> Result<(), String> {
    let EffectOp::GainLife { amount } = effect else {
        return Ok(());
    };
    let Some(filter) = amount.card_result_filter() else {
        return Ok(());
    };
    let Some((profile_filter, coefficient)) = program_gain_life_result_count_profile(amount) else {
        return Err(
            "Program GainLife result counts support only a direct result count or one positive zero-constant affine term".into(),
        );
    };
    debug_assert_eq!(filter, profile_filter);

    let input = input.ok_or_else(|| {
        "Program GainLife result count requires an explicitly bound input result".to_string()
    })?;
    let previous = previous.ok_or_else(|| {
        "Program GainLife result count requires the immediately preceding matching Discard or Controller Mill result".to_string()
    })?;
    let [previous_output] = previous.outputs.as_slice() else {
        return Err(
            "Program GainLife result count requires the immediately preceding matching Discard or Controller Mill result".into(),
        );
    };
    if *previous_output != input {
        return Err(
            "Program GainLife input must be the result of the immediately preceding instruction"
                .into(),
        );
    }

    let (action, players, producer_max) = match &previous.kind {
        CompiledInstructionKind::Perform(producer) => match producer.as_ref() {
            SpellEffectKind::ChooseHandCards {
                action: super::HandCardAction::Discard,
                count,
                ..
            } => (
                CardResultAction::Discard,
                RelativePlayerSet::All,
                Some(*count),
            ),
            SpellEffectKind::Mill {
                count,
                who: PlayerRecipient::Controller,
            } => (
                CardResultAction::Mill,
                RelativePlayerSet::Controller,
                amount_upper_bound(count),
            ),
            _ => {
                return Err(
                    "Program GainLife result count requires the immediately preceding matching Discard or Controller Mill result".into(),
                );
            }
        },
        _ => {
            return Err(
                "Program GainLife result count requires the immediately preceding matching Discard or Controller Mill result".into(),
            );
        }
    };

    if filter.source != CardResultSource::PreviousEffect
        || filter.action != action
        || filter.players != players
    {
        return Err(
            "Program GainLife result count must match the immediately preceding Discard or Controller Mill action and player set".into(),
        );
    }

    let slot = slots
        .get(input.0 as usize)
        .ok_or_else(|| "Program GainLife input result slot is unavailable".to_string())?;
    let result_max = match (&slot.result_type, action) {
        (
            ResultType::TargetCardResultCohort {
                action: CardResultAction::Discard,
                cardinality:
                    Cardinality {
                        min: 0,
                        max: Some(max),
                    },
                ..
            },
            CardResultAction::Discard,
        ) => Some(*max),
        (
            ResultType::CardResultCohort {
                action: CardResultAction::Mill,
                who: PlayerRecipient::Controller,
                cardinality:
                    Cardinality {
                        min: 0,
                        max: Some(max),
                    },
            },
            CardResultAction::Mill,
        ) => Some(*max),
        _ => None,
    };
    let Some(max) = result_max.filter(|max| producer_max == Some(*max)) else {
        return Err(
            "Program GainLife result count requires a finite matching Discard or Controller Mill result".into(),
        );
    };
    if coefficient.checked_mul(max).is_none() {
        return Err("Program GainLife result count can exceed u32::MAX".into());
    }
    Ok(())
}

fn validate_previous_mill_mana_value_sum_input(
    effect: &EffectOp,
    input: Option<ResultSlotId>,
    previous: Option<&CompiledInstruction>,
    slots: &[CompiledSlot],
) -> Result<(), String> {
    if !is_targeted_opponent_previous_mill_damage(effect) {
        return Ok(());
    }
    let slot_id = input.ok_or_else(|| {
        "PreviousMillManaValueSum requires an explicitly bound input from an immediately preceding Controller Mill instruction".to_string()
    })?;
    let previous = previous.ok_or_else(|| {
        "PreviousMillManaValueSum requires an immediately preceding Controller Mill instruction in the same program block".to_string()
    })?;
    if previous.outputs.len() != 1 || previous.outputs[0] != slot_id {
        return Err(
            "PreviousMillManaValueSum input must be the result bound by the immediately preceding Controller Mill instruction".into(),
        );
    }
    if !matches!(
        &previous.kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::Mill {
                who: PlayerRecipient::Controller,
                ..
            })
    ) {
        return Err(
            "PreviousMillManaValueSum requires an immediately preceding Controller Mill instruction".into(),
        );
    }
    if !matches!(
        slots.get(slot_id.0 as usize).map(|slot| &slot.result_type),
        Some(ResultType::CardResultCohort {
            action: CardResultAction::Mill,
            who: PlayerRecipient::Controller,
            ..
        })
    ) {
        return Err(
            "PreviousMillManaValueSum input must be a Controller Mill card-result cohort".into(),
        );
    }
    Ok(())
}

fn is_koya_outer_exile_delayed_consumer(effect: &EffectOp) -> bool {
    matches!(
        effect,
        EffectOp::CreateDelayedTrigger {
            subject: Some(EffectSubject::PreviousEffectObject),
            affected_player: None,
            ability,
        } if ability.ability_id.as_str() == "delayed_01"
            && ability.trigger == super::TriggerCondition::AtBeginningOfNextEndStep
    )
}

fn is_koya_outer_exile_producer(effect: &EffectOp) -> bool {
    matches!(
        effect,
        EffectOp::Exile {
            subject: EffectSubject::Chosen(filter),
            linked_exile_id: None,
        } if is_exact_koya_other_creature_filter(filter)
    )
}

fn validate_koya_exile_delayed_input(
    effect: &EffectOp,
    input: Option<ResultSlotId>,
    previous: Option<&CompiledInstruction>,
    slots: &[CompiledSlot],
    allow_koya_outer_exile_result: bool,
) -> Result<(), String> {
    if !allow_koya_outer_exile_result || !is_koya_outer_exile_delayed_consumer(effect) {
        return Ok(());
    }
    let invalid = || {
        "Koya delayed Exile consumer requires the immediately preceding exact optional Exile result"
            .to_string()
    };
    let Some(input) = input else {
        return Err(invalid());
    };
    let Some(previous) = previous else {
        return Err(invalid());
    };
    let [previous_output] = previous.outputs.as_slice() else {
        return Err(invalid());
    };
    if *previous_output != input
        || !matches!(
            &previous.kind,
            CompiledInstructionKind::Perform(previous_effect)
                if is_koya_outer_exile_producer(previous_effect)
        )
        || !matches!(
            slots.get(input.0 as usize).map(|slot| &slot.result_type),
            Some(ResultType::ExiledObject {
                cardinality: Cardinality {
                    min: 0,
                    max: Some(1)
                }
            })
        )
    {
        return Err(invalid());
    }
    Ok(())
}

fn validate_return_to_owners_hand_input(
    effect: &EffectOp,
    input: Option<ResultSlotId>,
    slots: &[CompiledSlot],
) -> Result<(), String> {
    let invalid = || {
        "library resolution requires a compatible singleton PermanentSelection input".to_string()
    };
    let selection_input = match effect {
        EffectOp::ReturnToOwnersHand {
            subject: EffectSubject::PreviousEffectObject,
        } => input.ok_or_else(invalid)?,
        _ => return Ok(()),
    };

    let Some(ResultType::PermanentSelection {
        cardinality: Cardinality { max: Some(1), .. },
        ..
    }) = slots
        .get(selection_input.0 as usize)
        .map(|slot| &slot.result_type)
    else {
        return Err(invalid());
    };
    Ok(())
}

fn compile_input(
    effect: &EffectOp,
    input: Option<&str>,
    names: &HashMap<String, ResultSlotId>,
    slots: &[CompiledSlot],
    allow_koya_outer_exile_result: bool,
) -> Result<Option<ResultSlotId>, String> {
    let token_copy_batch_consumer = is_token_copy_batch_consumer(effect);
    let expected = match effect {
        EffectOp::UntapChosenPermanents => Some("PermanentSelection"),
        EffectOp::GrantKeywords {
            subject: EffectSubject::PreviousEffectObject,
            ..
        } => Some("PermanentSelection"),
        EffectOp::ReturnToOwnersHand {
            subject: EffectSubject::PreviousEffectObject,
        } => Some("PermanentSelection"),
        EffectOp::AttachEquipment {
            equipment: EffectSubject::Source,
            creature: EffectSubject::PreviousEffectObject,
        } => Some("ManifestedPermanent"),
        EffectOp::AttachEquipment {
            equipment: EffectSubject::Source,
            creature: EffectSubject::PreviousEffectObjects,
        }
        | EffectOp::AttachEquipment {
            equipment: EffectSubject::PreviousEffectObject,
            creature: EffectSubject::Chosen(_),
        } => Some("PermanentSelection"),
        EffectOp::PutSelectedHandCardsOnLibraryTop => Some("HandSelection"),
        EffectOp::RemoveAllAbilities {
            subject: EffectSubject::PreviousEffectObject,
            duration: ResolvingEffectDuration::WhileSourceOnBattlefield,
        } => Some("CounteredAbilitySource"),
        EffectOp::ChooseCard {
            source: ChooseCardSource::CardResultCohort { .. },
            ..
        } => Some("CardResultCohort"),
        EffectOp::GainLife { amount } if amount.card_result_filter().is_some() => {
            Some("CardResultCohort")
        }
        EffectOp::RevealCohort => Some("FoundLibraryCohort"),
        EffectOp::ChooseCohortPartition { .. } => Some("RevealedLibraryCohort"),
        EffectOp::MoveCohortPartition { .. } => Some("CardPartition"),
        EffectOp::ReturnOtherLinkedExiledCards { .. } => Some("TriggerObjectLinkedExileReceipt"),
        _ if allow_koya_outer_exile_result && is_koya_outer_exile_delayed_consumer(effect) => {
            Some("ExiledObject")
        }
        EffectOp::CreateReflexiveTrigger { .. }
            if is_source_sacrifice_reflexive_consumer(effect) =>
        {
            Some("CardResultCohort")
        }
        EffectOp::DamagePlayer { .. } if is_targeted_opponent_previous_mill_damage(effect) => {
            Some("CardResultCohort")
        }
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
                EffectOp::GainLife { amount } if amount.card_result_filter().is_some() => {
                    matches!(
                        actual.result_type,
                        ResultType::CardResultCohort {
                            action: CardResultAction::Mill,
                            ..
                        } | ResultType::TargetCardResultCohort {
                            action: CardResultAction::Discard,
                            ..
                        }
                    )
                }
                EffectOp::RevealCohort => {
                    matches!(actual.result_type, ResultType::FoundLibraryCohort)
                }
                EffectOp::ChooseCohortPartition { .. } => {
                    matches!(actual.result_type, ResultType::RevealedLibraryCohort)
                }
                EffectOp::MoveCohortPartition { .. } => {
                    matches!(actual.result_type, ResultType::CardPartition)
                }
                EffectOp::ReturnOtherLinkedExiledCards {
                    producer_ability_id,
                    linked_exile_id,
                } => matches!(
                    &actual.result_type,
                    ResultType::TriggerObjectLinkedExileReceipt {
                        producer_ability_id: receipt_producer,
                        linked_exile_id: receipt_link,
                        cardinality: Cardinality {
                            min: 0,
                            max: Some(1),
                        },
                    } if receipt_producer == producer_ability_id
                        && receipt_link == linked_exile_id
                ),
                _ if allow_koya_outer_exile_result
                    && is_koya_outer_exile_delayed_consumer(effect) =>
                {
                    matches!(
                        actual.result_type,
                        ResultType::ExiledObject {
                            cardinality: Cardinality {
                                min: 0,
                                max: Some(1),
                            },
                        }
                    )
                }
                EffectOp::RemoveAllAbilities {
                    subject: EffectSubject::PreviousEffectObject,
                    duration: ResolvingEffectDuration::WhileSourceOnBattlefield,
                } => matches!(
                    &actual.result_type,
                    ResultType::CounteredAbilitySource {
                        cardinality: Cardinality {
                            min: 0,
                            max: Some(1),
                        },
                    }
                ),
                EffectOp::AttachEquipment {
                    equipment: EffectSubject::Source,
                    creature: EffectSubject::PreviousEffectObject,
                } => matches!(
                    actual.result_type,
                    ResultType::ManifestedPermanent {
                        cardinality: Cardinality {
                            min: 0,
                            max: Some(1),
                        },
                    }
                ),
                EffectOp::AttachEquipment {
                    equipment: EffectSubject::Source,
                    creature: EffectSubject::PreviousEffectObjects,
                } => matches!(
                    &actual.result_type,
                    ResultType::CreatedTokenBatch {
                        cardinality: Cardinality { min: 0, max: None },
                    }
                ),
                EffectOp::AttachEquipment {
                    equipment: EffectSubject::PreviousEffectObject,
                    creature: EffectSubject::Chosen(_),
                } => matches!(
                    &actual.result_type,
                    ResultType::PermanentSelection {
                        chooser: PlayerRecipient::Controller,
                        filter,
                        cardinality: Cardinality {
                            min: 0,
                            max: Some(1),
                        },
                    } if filter.as_ref() == &vow_equipment_filter()
                ),
                EffectOp::GrantKeywords {
                    subject: EffectSubject::PreviousEffectObject,
                    ..
                } => matches!(
                    actual.result_type,
                    ResultType::PermanentSelection {
                        cardinality: Cardinality {
                            min: 0,
                            max: Some(1),
                        },
                        ..
                    }
                ),
                EffectOp::CreateReflexiveTrigger { .. }
                    if is_source_sacrifice_reflexive_consumer(effect) =>
                {
                    matches!(
                        actual.result_type,
                        ResultType::CardResultCohort {
                            action: CardResultAction::Sacrifice,
                            who: PlayerRecipient::Controller,
                            cardinality: Cardinality {
                                min: 0,
                                max: Some(1),
                            },
                        }
                    )
                }
                EffectOp::DamagePlayer { .. }
                    if is_targeted_opponent_previous_mill_damage(effect) =>
                {
                    matches!(
                        actual.result_type,
                        ResultType::CardResultCohort {
                            action: CardResultAction::Mill,
                            who: PlayerRecipient::Controller,
                            ..
                        }
                    )
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

fn is_kyoshi_battle_fan_receipt_owner(owner: &ProgramOwnerId) -> bool {
    matches!(
        owner,
        ProgramOwnerId::Ability(definition)
            if definition.card_id == "kyoshi_battle_fan"
                && definition.face_id.as_str() == "kyoshi_battle_fan"
                && definition.ability_path.len() == 1
                && definition.ability_path[0].as_str() == "triggered_01"
    )
}

fn is_vow_to_erebor_face(owner: &ProgramOwnerId) -> bool {
    matches!(
        owner,
        ProgramOwnerId::Face(face)
            if face.card_id == "vow_to_erebor"
                && face.face_id.as_str() == "vow_to_erebor"
    )
}

fn is_exact_vow_target_filter(filter: &TargetFilter) -> bool {
    filter
        == &TargetFilter {
            kind: TargetKind::Creature,
            controller: super::TargetController::You,
            ..TargetFilter::default()
        }
}

fn vow_equipment_filter() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::AnyPermanent,
        controller: super::TargetController::You,
        required_subtypes: vec!["Equipment".into()],
        ..TargetFilter::default()
    }
}

fn is_exact_vow_dwarf_target_guard(condition: &GameCondition) -> bool {
    matches!(
        condition,
        GameCondition::ObjectMatches { object, filter }
            if *object == super::ConditionObjectRef::ChosenTarget {
                group_index: 0,
                target_index: 0,
            }
                && filter.as_ref()
                    == &TargetFilter {
                        kind: TargetKind::Creature,
                        required_subtypes: vec!["Dwarf".into()],
                        ..TargetFilter::default()
                    }
    )
}

fn is_exact_vow_equipment_choice(effect: &EffectOp) -> bool {
    matches!(
        effect,
        SpellEffectKind::ChoosePermanents {
            chooser: PlayerRecipient::Controller,
            filter,
            min: 0,
            max: 1,
            constraints,
        } if filter == &vow_equipment_filter()
            && constraints.as_slice()
                == [super::PermanentChoiceConstraint::EquipmentAttachableTo {
                    recipient: super::ConditionObjectRef::ChosenTarget {
                        group_index: 0,
                        target_index: 0,
                    },
                }]
    )
}

fn is_vow_chosen_target_effect(effect: &EffectOp) -> bool {
    matches!(
        effect,
        SpellEffectKind::Untap {
            subject: EffectSubject::Chosen(filter),
        } if is_exact_vow_target_filter(filter)
    ) || matches!(
        effect,
        SpellEffectKind::PumpTarget {
            power: 2,
            toughness: 2,
            subject: EffectSubject::Chosen(filter),
            ..
        } if is_exact_vow_target_filter(filter)
    ) || matches!(
        effect,
        SpellEffectKind::AttachEquipment {
            equipment: EffectSubject::PreviousEffectObject,
            creature: EffectSubject::Chosen(filter),
        } if is_exact_vow_target_filter(filter)
    )
}

fn validate_equipment_selection_input(
    effect: &EffectOp,
    input: Option<ResultSlotId>,
    previous: Option<&CompiledInstruction>,
    slots: &[CompiledSlot],
    allow_equipment_receipt: bool,
    allow_vow_equipment_choice: bool,
) -> Result<(), String> {
    match effect {
        SpellEffectKind::AttachEquipment {
            equipment: EffectSubject::Source,
            creature: EffectSubject::PreviousEffectObjects,
        } => {
            if !allow_equipment_receipt {
                return Err("CreatedTokenBatch attachment is reserved for Kyoshi Battle Fan's exact triggered Program".into());
            }
            let slot = input.ok_or_else(|| {
                "Kyoshi Battle Fan receipt attachment requires its immediately preceding CreatedTokenBatch input".to_string()
            })?;
            let previous = previous.ok_or_else(|| {
                "Kyoshi Battle Fan receipt attachment requires an immediately preceding CreateTokens instruction".to_string()
            })?;
            if previous.outputs.as_slice() != [slot]
                || !matches!(
                    &previous.kind,
                    CompiledInstructionKind::Perform(producer)
                        if matches!(
                            producer.as_ref(),
                            SpellEffectKind::CreateTokens {
                                token,
                                count: Amount::Fixed(1),
                                who: PlayerRecipient::Controller,
                                tapped: false,
                                sacrifice_timing: None,
                            } if token == "ally_w_1_1"
                        )
                )
                || !matches!(
                    slots.get(slot.0 as usize).map(|slot| &slot.result_type),
                    Some(ResultType::CreatedTokenBatch {
                        cardinality: Cardinality { min: 0, max: None }
                    })
                )
            {
                return Err("Kyoshi Battle Fan receipt attachment must consume the exact immediately preceding Ally CreateTokens result".into());
            }
        }
        SpellEffectKind::AttachEquipment {
            equipment: EffectSubject::PreviousEffectObject,
            creature: EffectSubject::Chosen(filter),
        } if is_exact_vow_target_filter(filter) => {
            if !allow_vow_equipment_choice {
                return Err(
                    "Vow to Erebor Equipment selection is reserved for its exact face Program"
                        .into(),
                );
            }
            let slot = input.ok_or_else(|| {
                "Vow to Erebor attachment requires its immediately preceding optional Equipment selection".to_string()
            })?;
            let previous = previous.ok_or_else(|| {
                "Vow to Erebor attachment requires an immediately preceding ChoosePermanents instruction".to_string()
            })?;
            if previous.outputs.as_slice() != [slot]
                || !matches!(
                    &previous.kind,
                    CompiledInstructionKind::Perform(producer)
                        if is_exact_vow_equipment_choice(producer.as_ref())
                )
                || !matches!(
                    slots.get(slot.0 as usize).map(|slot| &slot.result_type),
                    Some(ResultType::PermanentSelection {
                        chooser: PlayerRecipient::Controller,
                        filter,
                        cardinality: Cardinality { min: 0, max: Some(1) },
                    }) if filter.as_ref() == &vow_equipment_filter()
                )
            {
                return Err("Vow to Erebor attachment must consume only its exact immediately preceding Equipment selection".into());
            }
        }
        _ => {}
    }
    Ok(())
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
            ResultType::CounteredAbilitySource { cardinality: a },
            ResultType::CounteredAbilitySource { cardinality: b },
        ) => ResultType::CounteredAbilitySource {
            cardinality: union(*a, *b),
        },
        (
            ResultType::ExiledObject { cardinality: a },
            ResultType::ExiledObject { cardinality: b },
        ) => ResultType::ExiledObject {
            cardinality: union(*a, *b),
        },
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
        (
            ResultType::LinkedExileCardResultCohort {
                producer_ability_id: ap,
                linked_exile_id: al,
                cardinality: ac,
            },
            ResultType::LinkedExileCardResultCohort {
                producer_ability_id: bp,
                linked_exile_id: bl,
                cardinality: bc,
            },
        ) if ap == bp && al == bl => ResultType::LinkedExileCardResultCohort {
            producer_ability_id: ap.clone(),
            linked_exile_id: al.clone(),
            cardinality: union(*ac, *bc),
        },
        (
            ResultType::TargetCardResultCohort {
                action: aa,
                cardinality: ac,
                target_group_index: ag,
            },
            ResultType::TargetCardResultCohort {
                action: ba,
                cardinality: bc,
                target_group_index: bg,
            },
        ) if aa == ba && ag == bg => ResultType::TargetCardResultCohort {
            action: *aa,
            cardinality: union(*ac, *bc),
            target_group_index: *ag,
        },
        (
            ResultType::CastPaymentCohort {
                action: aa,
                cardinality: ac,
            },
            ResultType::CastPaymentCohort {
                action: ba,
                cardinality: bc,
            },
        ) if aa == ba => ResultType::CastPaymentCohort {
            action: *aa,
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
        (
            ResultType::CreatedTokenBatch { cardinality: a },
            ResultType::CreatedTokenBatch { cardinality: b },
        ) if a == b => ResultType::CreatedTokenBatch { cardinality: *a },
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
    target_group_index: Option<u32>,
    target_group_max: Option<u32>,
    compiler: &Compiler<'_>,
) -> Result<Vec<ResultType>, String> {
    match effect {
        SpellEffectKind::CounterTargetAbility
            if matches!(context, EffectContext::Spell | EffectContext::Ability) =>
        {
            if target_group_index.is_none() || target_group_max != Some(1) {
                return Err(
                    "CounterTargetAbility requires one optional or required single-target group"
                        .into(),
                );
            }
            effect.validate(context)?;
            Ok(vec![ResultType::CounteredAbilitySource {
                cardinality: Cardinality {
                    min: 0,
                    max: Some(1),
                },
            }])
        }
        SpellEffectKind::CounterTargetAbility => Err(
            "CounterTargetAbility in a resolution program requires a spell or ability owner"
                .into(),
        ),
        SpellEffectKind::ManifestDread if context == EffectContext::Ability => {
            effect.validate(context)?;
            Ok(vec![ResultType::ManifestedPermanent {
                cardinality: Cardinality {
                    min: 0,
                    max: Some(1),
                },
            }])
        }
        SpellEffectKind::ManifestDread => Err(
            "ManifestDread in a resolution program requires an ability-owned trigger".into(),
        ),
        SpellEffectKind::AttachEquipment {
            equipment: EffectSubject::Source,
            creature: EffectSubject::PreviousEffectObject,
        } => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::AttachEquipment {
            equipment: EffectSubject::Source,
            creature: EffectSubject::PreviousEffectObjects,
        } if compiler.allow_equipment_receipt && context == EffectContext::Ability => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::AttachEquipment {
            equipment: EffectSubject::PreviousEffectObject,
            creature: EffectSubject::Chosen(filter),
        } if compiler.allow_vow_equipment_choice && is_exact_vow_target_filter(filter) => {
            effect.validate(context)?;
            validate_program_target_filter(filter, "Vow to Erebor attachment target")?;
            Ok(Vec::new())
        }
        SpellEffectKind::Untap {
            subject: EffectSubject::Chosen(filter),
        } if compiler.allow_vow_equipment_choice && is_exact_vow_target_filter(filter) => {
            effect.validate(context)?;
            validate_program_target_filter(filter, "Vow to Erebor untap target")?;
            Ok(Vec::new())
        }
        SpellEffectKind::PumpTarget {
            power: 2,
            toughness: 2,
            subject: EffectSubject::Chosen(filter),
            ..
        } if compiler.allow_vow_equipment_choice && is_exact_vow_target_filter(filter) => {
            effect.validate(context)?;
            validate_program_target_filter(filter, "Vow to Erebor pump target")?;
            Ok(Vec::new())
        }
        SpellEffectKind::AttachEquipment { .. } => Err(
            "AttachEquipment in a resolution program requires Source and PreviousEffectObject".into(),
        ),
        SpellEffectKind::MoveGraveyardCards {
            filter,
            destination: GraveyardDestination::Exile,
            linked_exile_id,
            entry_modifiers,
        } if context == EffectContext::Ability => {
            let target_group_index = target_group_index.ok_or_else(|| {
                "ability_program MoveGraveyardCards Exile requires one explicit Target binding"
                    .to_string()
            })?;
            let max = target_group_max.ok_or_else(|| {
                "ability_program graveyard Exile result requires a validated target group".to_string()
            })?;
            if linked_exile_id.is_some() || !entry_modifiers.is_empty() {
                return Err(
                    "ability_program MoveGraveyardCards Exile does not support linked exile or entry modifiers".into(),
                );
            }
            if filter.mana_value_less_than_trigger_object {
                return Err(
                    "ability_program MoveGraveyardCards Exile cannot depend on a trigger object".into(),
                );
            }
            effect.validate(context)?;
            Ok(vec![ResultType::TargetCardResultCohort {
                action: CardResultAction::Exile,
                cardinality: Cardinality {
                    min: 0,
                    max: Some(max),
                },
                target_group_index,
            }])
        }
        SpellEffectKind::MoveGraveyardCards { .. } => Err(
            "MoveGraveyardCards in a resolution program supports only a targeted ability-owned graveyard Exile".into(),
        ),
        SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(filter),
        } if context == EffectContext::Ability
            && filter.as_ref()
                == &(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    permanent_types: vec![super::PermanentTypeFilter::Artifact],
                    ..TargetFilter::default()
                }) =>
        {
            effect.validate(context)?;
            validate_program_target_filter(filter, "Destroy target")?;
            Ok(Vec::new())
        }
        SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(filter),
        } if context == EffectContext::Spell => {
            let target_group_index = target_group_index.ok_or_else(|| {
                "spell_program Destroy requires exactly one explicit Subject target binding".to_string()
            })?;
            effect.validate(context)?;
            validate_program_target_filter(filter, "Destroy target")?;
            Ok(vec![ResultType::TargetCardResultCohort {
                action: CardResultAction::Destroy,
                cardinality: Cardinality {
                    min: 0,
                    max: Some(1),
                },
                target_group_index,
            }])
        }
        SpellEffectKind::PutInOwnersLibrary {
            subject: EffectSubject::Chosen(filter),
            ..
        } if context == EffectContext::Spell => {
            if target_group_index.is_none() || target_group_max != Some(1) {
                return Err(
                    "PutInOwnersLibrary Chosen requires one explicit single-target binding".into(),
                );
            }
            effect.validate(context)?;
            validate_program_target_filter(filter, "PutInOwnersLibrary target")?;
            Ok(Vec::new())
        }
        SpellEffectKind::PutInOwnersLibrary {
            subject: EffectSubject::AttachedObject,
            placement: super::LibraryPlacement::Shuffle,
        } if context == EffectContext::Ability => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::PutInOwnersLibrary { .. } => Err(
            "PutInOwnersLibrary in a resolution program supports a bound single chosen target, or an ability-owned AttachedObject Shuffle".into(),
        ),
        SpellEffectKind::Destroy { .. } if context == EffectContext::Spell => Err(
            "spell_program Destroy requires a chosen permanent target with one explicit Subject binding".into(),
        ),
        SpellEffectKind::Destroy { .. } => Err(
            "Destroy in an ability program supports only a chosen Artifact permanent target".into(),
        ),
        SpellEffectKind::ChooseHandCards {
            action,
            count,
            target,
            chooser,
            card_filter,
            optional,
            visibility,
        } if context == EffectContext::Spell => {
            let exact_target = TargetFilter {
                kind: TargetKind::AnyPlayer,
                ..TargetFilter::default()
            };
            if *action != super::HandCardAction::Discard
                || *count == 0
                || target != &exact_target
                || *chooser != super::HandCardChooser::AffectedPlayer
                || card_filter.is_some()
                || *optional
                || *visibility != super::HandChoiceVisibility::PrivateLook
            {
                return Err(
                    "ChooseHandCards in spell_program supports only a positive, unfiltered, nonoptional affected-player discard from one AnyPlayer target".into(),
                );
            }
            let target_group_index = target_group_index.ok_or_else(|| {
                "spell_program ChooseHandCards requires exactly one explicit Target binding"
                    .to_string()
            })?;
            effect.validate(context)?;
            validate_program_target_filter(target, "ChooseHandCards target")?;
            Ok(vec![ResultType::TargetCardResultCohort {
                action: CardResultAction::Discard,
                cardinality: Cardinality {
                    min: 0,
                    max: Some(*count),
                },
                target_group_index,
            }])
        }
        SpellEffectKind::ChooseHandCards { .. } => Err(
            "ChooseHandCards in a resolution program supports only the exact spell target-discard profile".into(),
        ),
        SpellEffectKind::Draw { who, count } => {
            effect.validate(context)?;
            validate_program_draw_recipient(*who, context)?;
            validate_program_amount(count)?;
            validate_fixed_program_amount("Draw count", count)?;
            // Replacement effects may expand the committed draw cohort, so the authored count
            // does not bound the receipt.
            Ok(vec![ResultType::DrawReceipt {
                who: *who,
                cardinality: Cardinality { min: 0, max: None },
            }])
        }
        SpellEffectKind::Scry { count } => {
            validate_program_amount(count)?;
            validate_fixed_program_amount("Scry count", count)?;
            // Legacy Scry rejects a literal zero, but Programs admit it as a resultless no-op.
            // Keep the ordinary validator for positive counts and preserve its context checks.
            if !matches!(count, Amount::Fixed(0)) {
                effect.validate(context)?;
            }
            Ok(Vec::new())
        }
        SpellEffectKind::LookChooseToHand {
            filter,
            min,
            max,
            reveal,
            bottom_order,
            ..
        } => {
            effect.validate(context)?;
            if context != EffectContext::Spell {
                return Err("LookChooseToHand in spell_program is spell-only".into());
            }
            if filter.is_some()
                || *reveal
                || *bottom_order != LibraryBottomOrder::Chosen
                || *min == 0
                || min != max
            {
                return Err(
                    "LookChooseToHand in spell_program requires an exact positive unfiltered, unrevealed, chosen-order selection".into(),
                );
            }
            Ok(Vec::new())
        }
        SpellEffectKind::GainLife { amount } if amount.card_result_filter().is_some() => {
            if program_gain_life_result_count_profile(amount).is_none() {
                return Err(
                    "Program GainLife result counts support only a direct result count or one positive zero-constant affine term".into(),
                );
            }
            // The direct leaf retains ordinary effect validation. The one-term affine form is
            // checked by the Program's immediate producer contract; global CountExpression
            // affine validation remains closed to result leaves.
            if matches!(amount, Amount::Count(CountExpression::CardsMatchingResult { .. })) {
                effect.validate(context)?;
            }
            Ok(Vec::new())
        }
        SpellEffectKind::GainLife { amount } => {
            effect.validate(context)?;
            validate_program_amount(amount)?;
            validate_fixed_program_amount("GainLife amount", amount)?;
            Ok(Vec::new())
        }
        SpellEffectKind::LoseLife { amount, who }
            if matches!(amount, LifeAmount::Fixed(_))
                && (*who == PlayerRecipient::EachOpponent
                    || (*who == PlayerRecipient::Controller
                        && matches!(context, EffectContext::Spell | EffectContext::Ability))) =>
        {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::LoseLife { .. } => Err(
            "LoseLife in a resolution program supports a fixed EachOpponent amount, or a fixed Controller amount on a spell or ability program".into(),
        ),
        SpellEffectKind::Blight { .. } => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::Proliferate => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::GrantKeywordsAll { filter, .. } => {
            effect.validate(context)?;
            if matches!(
                filter.controller,
                Some(super::CreatureScopeController::TargetedPlayer { .. })
            ) {
                return Err(
                    "GrantKeywordsAll in a resolution program does not support TargetedPlayer scopes without a target-role binding".into(),
                );
            }
            Ok(Vec::new())
        }
        SpellEffectKind::RemoveAllAbilities {
            subject: EffectSubject::PreviousEffectObject,
            duration: ResolvingEffectDuration::WhileSourceOnBattlefield,
        } if context == EffectContext::Ability => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::GrantKeywords {
            subject: EffectSubject::PreviousEffectObject,
            ..
        } if context == EffectContext::Spell => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::GrantKeywords {
            subject: EffectSubject::PreviousEffectObject,
            ..
        } => Err("GrantKeywords(PreviousEffectObject) in a resolution program is spell-only".into()),
        SpellEffectKind::RemoveAbilitiesAll { .. } | SpellEffectKind::DestroyAll { .. } => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::CreateReflexiveTrigger { .. } => {
            effect.validate(context)?;
            validate_resultless_program_reflexive_trigger(effect, context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::Discard { who, quantity } => {
            effect.validate(context)?;
            let player_set_discard = context == EffectContext::Ability
                && matches!(*who, PlayerRecipient::EachOpponent | PlayerRecipient::EachPlayer)
                && matches!(quantity, super::DiscardQuantity::Exact(1));
            if !player_set_discard {
                validate_program_recipient(*who, context, "Discard recipient")?;
            }
            let optional_payment = matches!(quantity, super::DiscardQuantity::OptionalExact(_));
            Ok(vec![ResultType::DiscardResult {
                who: *who,
                cardinality: Cardinality {
                    min: 0,
                    // Player-set recipients contribute one choice per applicable player to a
                    // combined typed cohort, so the per-player Exact(1) count is not an
                    // aggregate upper bound.
                    max: if player_set_discard {
                        None
                    } else {
                        discard_upper_bound(quantity)
                    },
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
            let targeted_group = targeted_opponent_singleton_choice_group(effect);
            if targeted_group.is_some() && context != EffectContext::Ability {
                return Err(
                    "targeted-opponent ChoosePermanents is supported only on an ability program"
                        .into(),
                );
            }
            if targeted_group.is_none() {
                validate_program_controller(*chooser, "ChoosePermanents chooser")?;
            } else if target_group_index != targeted_group {
                return Err(
                    "targeted-opponent ChoosePermanents must bind its player target to the authored target group"
                        .into(),
                );
            }
            validate_program_choice_filter(filter)?;
            if !(constraints.is_empty()
                || compiler.allow_vow_equipment_choice && is_exact_vow_equipment_choice(effect))
            {
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
        } => validate_resultless_conditional_top_search(effect, context),
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
        SpellEffectKind::LibraryPartition {
            count: 1,
            top_min: 0,
            top_max: None,
            kind: super::LibraryPartitionKind::Surveil,
        } if context == EffectContext::Spell => {
            effect.validate(context)?;
            Ok(Vec::new())
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
        SpellEffectKind::ExileTopWithPlayPermission {
            player: PlayerRecipient::Controller,
            count: 1,
            count_by_cast_cost: None,
        } if context == EffectContext::Spell => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::ExileTopWithPlayPermission { .. } => Err(
            "ExileTopWithPlayPermission in a resolution program supports only one Controller card without a cast-cost conditional on a spell".into(),
        ),
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
        SpellEffectKind::DamagePlayer {
            amount,
            who: PlayerRecipient::EachOpponent,
        } => {
            if context != EffectContext::Spell {
                return Err("DamagePlayer EachOpponent in spell_program is spell-only".into());
            }
            effect.validate(context)?;
            match amount {
                Amount::Fixed(value) if *value > 0 => Ok(Vec::new()),
                _ => Err(
                    "DamagePlayer EachOpponent in spell_program requires a positive fixed amount"
                        .into(),
                ),
            }
        }
        SpellEffectKind::DamagePlayer {
            amount,
            who:
                PlayerRecipient::TargetedPlayer {
                    kind: TargetKind::OpponentPlayer,
                    ..
                },
        } => {
            effect.validate(context)?;
            match amount {
                Amount::Fixed(value) if *value > 0 => Ok(Vec::new()),
                Amount::Count(super::CountExpression::PreviousMillManaValueSum) => {
                    amount.validate_effect(context)?;
                    Ok(Vec::new())
                }
                _ => Err(
                    "DamagePlayer TargetedPlayer OpponentPlayer in spell_program requires a positive fixed amount or a direct PreviousMillManaValueSum result"
                        .into(),
                ),
            }
        }
        SpellEffectKind::DamagePlayer {
            amount,
            who: PlayerRecipient::ControllerOfTargetGroup { .. },
        } => {
            if context != EffectContext::Spell {
                return Err(
                    "DamagePlayer ControllerOfTargetGroup in spell_program is spell-only".into(),
                );
            }
            effect.validate(context)?;
            validate_program_amount(amount)?;
            validate_fixed_program_amount("DamagePlayer amount", amount)?;
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
            match subject {
                super::EffectSubject::Chosen(filter) => {
                    validate_program_target_filter(filter, "ReturnToOwnersHand target")?;
                }
                super::EffectSubject::PreviousEffectObject => {}
                _ => {
                    return Err(
                        "ReturnToOwnersHand requires an owner-local chosen target or its exact typed selection input in a resolution program"
                            .into(),
                    );
                }
            }
            Ok(Vec::new())
        }
        SpellEffectKind::ShuffleNonlandPermanentsIntoOwnersLibraries { .. } => {
            effect.validate(context)?;
            if context != EffectContext::Ability
                || targeted_opponent_nonland_shuffle_group(effect).is_none()
                || target_group_index != targeted_opponent_nonland_shuffle_group(effect)
            {
                return Err(
                    "ShuffleNonlandPermanentsIntoOwnersLibraries supports only its authored targeted-opponent ability group in a resolution program"
                        .into(),
                );
            }
            Ok(Vec::new())
        }
        SpellEffectKind::ReturnAllToOwnersHand { .. } => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::CreateTokens {
            token,
            count,
            who,
            tapped,
            sacrifice_timing,
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
            validate_program_create_tokens_amount(count, who)?;
            Ok(if compiler.allow_equipment_receipt
                && token == "ally_w_1_1"
                && matches!(count, Amount::Fixed(1))
                && *who == PlayerRecipient::Controller
                && !*tapped
                && sacrifice_timing.is_none()
            {
                vec![ResultType::CreatedTokenBatch {
                    cardinality: Cardinality { min: 0, max: None },
                }]
            } else {
                Vec::new()
            })
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
                super::TokenCopySource::Source => {
                    return Err(
                        "CreateTokenCopies source is not supported in a resolution program".into(),
                    );
                }
                super::TokenCopySource::LinkedExile {
                    producer_ability_id,
                    linked_exile_id,
                } if context == EffectContext::Ability
                    && is_mimic_vat_create_copy_owner(&compiler.owner)
                    && producer_ability_id.as_str() == "imprint"
                    && linked_exile_id.as_str() == "imprinted_card"
                    && matches!(count, Amount::Fixed(1)) => {}
                super::TokenCopySource::LinkedExile { .. } => {
                    return Err(
                        "LinkedExile token copies require Mimic Vat's exact create_copy ability, imprint producer, imprinted_card link, and count 1".into(),
                    );
                }
            }
            Ok(if matches!(
                source,
                TokenCopySource::TriggerObject | TokenCopySource::LinkedExile { .. }
            ) {
                vec![ResultType::TokenCopyBatch]
            } else {
                Vec::new()
            })
        }
        SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(filter),
        } if context == EffectContext::Ability => {
            if target_group_index.is_none() || target_group_max != Some(1) {
                return Err("Tap requires one required single-target group".into());
            }
            effect.validate(context)?;
            validate_program_target_filter(filter, "Tap target")?;
            Ok(Vec::new())
        }
        SpellEffectKind::PutCounters {
            counter: super::CounterKind::Stun,
            count: Amount::Fixed(1),
            subject: EffectSubject::Chosen(filter),
        } if context == EffectContext::Ability
            && filter.kind == TargetKind::Creature
            && filter.any_of.is_none() =>
        {
            if target_group_index.is_none() || target_group_max != Some(1) {
                return Err("Stun PutCounters requires one required single-target group".into());
            }
            effect.validate(context)?;
            validate_program_target_filter(filter, "Stun PutCounters target")?;
            Ok(Vec::new())
        }
        SpellEffectKind::ShufflePermanentsIntoOwnersLibraries { subjects }
            if context == EffectContext::Ability =>
        {
            let Some(filter) = single_source_chosen_shuffle_filter(subjects) else {
                return Err(
                    "ShufflePermanentsIntoOwnersLibraries in a resolution program requires exactly Source and one Chosen permanent"
                        .into(),
                );
            };
            if target_group_index.is_none() || target_group_max != Some(1) {
                return Err(
                    "ShufflePermanentsIntoOwnersLibraries requires one required single-target group"
                        .into(),
                );
            }
            effect.validate(context)?;
            validate_program_target_filter(filter, "ShufflePermanentsIntoOwnersLibraries target")?;
            Ok(Vec::new())
        }
        SpellEffectKind::PutCounters {
            counter,
            count,
            subject: EffectSubject::Source,
        } if context == EffectContext::Ability
            && matches!(count, Amount::Fixed(amount) if *amount > 0)
        =>
        {
            effect.validate(context)?;
            counter.validate()?;
            Ok(Vec::new())
        }
        SpellEffectKind::PutCounters {
            counter: super::CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Chosen(filter),
        } if context == EffectContext::Spell && is_exact_creature_target_filter(filter) => {
            effect.validate(context)?;
            validate_program_target_filter(filter, "PutCounters target")?;
            Ok(Vec::new())
        }
        SpellEffectKind::RemoveAllCounters {
            counter,
            subject: EffectSubject::Source,
        } if context == EffectContext::Ability => {
            effect.validate(context)?;
            counter.validate()?;
            Ok(Vec::new())
        }
        SpellEffectKind::RemoveCounters {
            counter,
            count,
            subject: EffectSubject::Source,
        } if context == EffectContext::Ability && *count > 0 => {
            effect.validate(context)?;
            counter.validate()?;
            Ok(Vec::new())
        }
        SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(filter),
            linked_exile_id: None,
        } if context == EffectContext::Spell => {
            if target_group_index != Some(0)
                || target_group_max != Some(1)
                || !is_exact_creature_target_filter(filter)
            {
                return Err(
                    "spell_program Exile requires its singleton Creature target group 0".into(),
                );
            }
            effect.validate(context)?;
            validate_program_target_filter(filter, "Exile target")?;
            Ok(Vec::new())
        }
        SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(filter),
            linked_exile_id: None,
        } if context == EffectContext::Ability && compiler.allow_koya_outer_exile_result => {
            if target_group_index != Some(0)
                || target_group_max != Some(1)
                || !is_exact_koya_other_creature_filter(filter)
            {
                return Err(
                    "Koya triggered_01 Exile requires its optional other-Creature target group 0"
                        .into(),
                );
            }
            effect.validate(context)?;
            validate_program_target_filter(filter, "Koya Exile target")?;
            Ok(vec![ResultType::ExiledObject {
                cardinality: Cardinality {
                    min: 0,
                    max: Some(1),
                },
            }])
        }
        SpellEffectKind::Exile {
            subject: EffectSubject::TriggerObject,
            linked_exile_id: Some(linked_exile_id),
        } if context == EffectContext::Ability
            && compiler.allow_trigger_object_linked_exile
            && compiler.allow_mimic_vat_linked_exile_receipt =>
        {
            effect.validate(context)?;
            Ok(vec![ResultType::TriggerObjectLinkedExileReceipt {
                producer_ability_id: AbilityId::new("imprint")
                    .expect("the compiled Mimic Vat ability ID is canonical"),
                linked_exile_id: linked_exile_id.clone(),
                cardinality: Cardinality {
                    min: 0,
                    max: Some(1),
                },
            }])
        }
        SpellEffectKind::Exile {
            subject: EffectSubject::TriggerObject,
            linked_exile_id: Some(_),
        } if context == EffectContext::Ability && compiler.allow_trigger_object_linked_exile => {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::ReturnOtherLinkedExiledCards { .. }
            if context == EffectContext::Ability
                && compiler.allow_mimic_vat_linked_exile_receipt =>
        {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::ReturnOtherLinkedExiledCards { .. } => Err(
            "ReturnOtherLinkedExiledCards in a resolution program requires the immediately preceding Mimic Vat imprint receipt".into(),
        ),
        SpellEffectKind::ChooseLinkedExiledCard {
            producer_ability_id,
            linked_exile_id,
            destination: super::LinkedExiledCardDestination::OwnerGraveyard,
            optional: false,
            ..
        } if context == EffectContext::Ability => {
            effect.validate(context)?;
            Ok(vec![ResultType::LinkedExileCardResultCohort {
                producer_ability_id: producer_ability_id.clone(),
                linked_exile_id: linked_exile_id.clone(),
                cardinality: Cardinality {
                    min: 0,
                    max: Some(1),
                },
            }])
        }
        SpellEffectKind::ChooseLinkedExiledCard { .. } => Err(
            "ChooseLinkedExiledCard in a resolution program requires a mandatory ability-owned OwnerGraveyard choice"
                .into(),
        ),
        SpellEffectKind::PlayLinkedExiledCard {
            producer_ability_id,
            linked_exile_id,
            filter: None,
            play_kind: super::LinkedExiledCardPlayKind::Play,
            return_source_to_owner_hand_if_cast: false,
        } if context == EffectContext::Ability
            && compiler.allow_mosswort_linked_play
            && producer_ability_id.as_str() == "hideaway"
            && linked_exile_id.as_str() == "exiled_cards" =>
        {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::PlayLinkedExiledCard { .. } => Err(
            "PlayLinkedExiledCard in a resolution program requires the exact Mosswort Bridge activated_02 Play profile"
                .into(),
        ),
        SpellEffectKind::Sacrifice {
            subject: EffectSubject::Source,
        } if context == EffectContext::Ability => {
            effect.validate(context)?;
            Ok(vec![ResultType::CardResultCohort {
                action: CardResultAction::Sacrifice,
                who: PlayerRecipient::Controller,
                cardinality: Cardinality {
                    min: 0,
                    max: Some(1),
                },
            }])
        }
        SpellEffectKind::ApplyCombatRestriction {
            scope: super::CombatRestrictionScope::Source,
            restriction,
        } if context == EffectContext::Ability
            && restriction
                == &(super::CombatRestriction {
                    cant_be_blocked: true,
                    ..Default::default()
                }) =>
        {
            effect.validate(context)?;
            Ok(Vec::new())
        }
        SpellEffectKind::ApplyCombatRestriction {
            scope: super::CombatRestrictionScope::Chosen(filter),
            restriction,
        } if context == EffectContext::Spell
            && is_exact_creature_target_filter(filter)
            && restriction
                == &(super::CombatRestriction {
                    cant_be_blocked: true,
                    ..Default::default()
                }) =>
        {
            effect.validate(context)?;
            validate_program_target_filter(filter, "ApplyCombatRestriction target")?;
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
        SpellEffectKind::CounterTriggeringStackObjectUnlessPays { cost } => {
            if context != EffectContext::Ability {
                return Err(
                    "CounterTriggeringStackObjectUnlessPays in a resolution program requires an ability-owned trigger"
                        .into(),
                );
            }
            match cost {
                ResolutionCost::Mana(mana)
                    if !mana.pips.is_empty()
                        && mana.mana_value() > 0
                        && !mana.pips.iter().any(|pip| matches!(pip, crate::ManaSymbol::X)) =>
                {
                }
                ResolutionCost::DiscardCard { filter: None } => {}
                _ => {
                    return Err(
                        "CounterTriggeringStackObjectUnlessPays in a resolution program supports only fixed nonempty no-X mana or an unfiltered discard-card cost"
                            .into(),
                    )
                }
            }
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
        _ if compiler.allow_koya_outer_exile_result
            && is_koya_outer_exile_delayed_consumer(effect) =>
        {
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

fn validate_resultless_program_reflexive_trigger(
    effect: &SpellEffectKind,
    context: EffectContext,
) -> Result<(), String> {
    let SpellEffectKind::CreateReflexiveTrigger { when, ability } = effect else {
        return Err("expected a CreateReflexiveTrigger program leaf".into());
    };
    if context != EffectContext::Ability {
        return Err(
            "CreateReflexiveTrigger in a resolution program requires an ability-owned permanent source"
                .into(),
        );
    }
    if ability.targeting.is_some() || ability.intervening_if.is_some() {
        return Err(
            "CreateReflexiveTrigger supports only the approved resultless target shapes".into(),
        );
    }

    if is_source_sacrifice_reflexive_consumer(effect) {
        if let [SpellEffectKind::DamageAll {
            amount: Amount::Fixed(amount),
            players: super::MassPlayerSet::All,
            kind,
        }] = ability.effect.as_slice()
        {
            if *amount > 0 && kind == &TargetFilter::default_creature() {
                return Ok(());
            }
        }
        return Err(
            "Sacrificed(Source) reflexive programs support only positive fixed damage to all creatures"
                .into(),
        );
    }
    if when.is_some() {
        return Err(
            "CreateReflexiveTrigger supports only the approved resultless target shapes or the immediate Source sacrifice receipt".into(),
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

    if let [SpellEffectKind::DamageTarget { amount, target }] = ability.effect.as_slice() {
        if !matches!(amount, Amount::Fixed(_)) {
            return Err("CreateReflexiveTrigger DamageTarget supports only a fixed amount".into());
        }
        validate_program_target_filter(target, "CreateReflexiveTrigger DamageTarget target")?;
        return Ok(());
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
        filter,
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
        && resultless_search_bounds_match(filter, *selection_bounds)
}

fn resultless_search_bounds_match(
    filter: &Option<ZoneCardFilter>,
    selection_bounds: Option<SearchSelectionBounds>,
) -> bool {
    let min = if filter.is_some() { 0 } else { 1 };
    selection_bounds == Some(SearchSelectionBounds { min, max: 1 })
}

fn is_resultless_direct_top_library_search(effect: &SpellEffectKind) -> bool {
    let SpellEffectKind::SearchLibrary { destination, .. } = effect else {
        return false;
    };

    is_resultless_direct_search_base(effect) && *destination == SearchDestination::TopOfLibrary
}

fn validate_resultless_direct_top_library_search(
    effect: &SpellEffectKind,
    context: EffectContext,
) -> Result<Vec<ResultType>, String> {
    if !is_resultless_direct_top_library_search(effect) {
        return Err(
            "program direct TopOfLibrary SearchLibrary supports only a resultless controller search with a validated filter or None, fixed count 1, qualified/unrestricted bounds, and shuffle"
                .into(),
        );
    }

    validate_resultless_direct_search(effect, context)
}

fn is_resultless_direct_hand_library_search(effect: &SpellEffectKind) -> bool {
    let SpellEffectKind::SearchLibrary { destination, .. } = effect else {
        return false;
    };

    is_resultless_direct_search_base(effect) && *destination == SearchDestination::Hand
}

fn validate_resultless_direct_hand_library_search(
    effect: &SpellEffectKind,
    context: EffectContext,
) -> Result<Vec<ResultType>, String> {
    if !is_resultless_direct_hand_library_search(effect) {
        return Err(
            "program direct Hand SearchLibrary supports only a resultless controller search with a validated filter or None, fixed count 1, qualified/unrestricted bounds, and shuffle"
                .into(),
        );
    }

    // The authored bounds are consumed by this one-card hand movement. The normal direct effect
    // validator still checks the actor, filter, zone, count, and destination contracts.
    validate_resultless_direct_search(effect, context)
}

fn is_resultless_direct_battlefield_library_search(effect: &SpellEffectKind) -> bool {
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

    let count_and_bounds_match = (*count == 1
        && *selection_bounds == Some(SearchSelectionBounds { min: 0, max: 1 }))
        || (*count == 2 && *selection_bounds == Some(SearchSelectionBounds { min: 0, max: 2 }));

    *who == PlayerRecipient::Controller
        && !*optional
        && count_and_bounds_match
        && count_by_cast_cost.is_none()
        && filter.as_ref().is_some_and(|filter| {
            *filter
                == ZoneCardFilter {
                    card_type: Some(CardTypeFilter::BasicLand),
                    ..ZoneCardFilter::default()
                }
        })
        && selection_constraint.is_none()
        && slots.is_empty()
        && *zones == SearchZoneSelection::Fixed(vec![CardSearchZone::Library])
        && *destination == (SearchDestination::Battlefield { tapped: true })
        && conditional_destination.is_none()
        && *shuffle
        && !*reveal
        && result_id.is_none()
}

fn validate_resultless_direct_battlefield_library_search(
    effect: &SpellEffectKind,
    context: EffectContext,
) -> Result<Vec<ResultType>, String> {
    if !is_resultless_direct_battlefield_library_search(effect) {
        return Err(
            "program direct Battlefield SearchLibrary supports only a resultless controller BasicLand search with count 1/bounds 0..1 or count 2/bounds 0..2, tapped destination, no reveal, and shuffle"
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

/// Keep conditional resultless search to the existing TopOfLibrary-to-tapped-Battlefield
/// transaction while validating its predicate through the same live condition domain as ProgramIf.
fn validate_resultless_conditional_top_search(
    effect: &SpellEffectKind,
    context: EffectContext,
) -> Result<Vec<ResultType>, String> {
    if !is_resultless_conditional_top_search(effect) {
        return Err(
            "program conditional TopOfLibrary SearchLibrary supports only a resultless controller BasicLand search to a tapped Battlefield destination, fixed count 1, bounds 0..1, reveal and shuffle"
                .into(),
        );
    }

    let SpellEffectKind::SearchLibrary {
        conditional_destination: Some(conditional_destination),
        ..
    } = effect
    else {
        unreachable!("the validated conditional search shape has a destination predicate")
    };
    validate_program_condition(&conditional_destination.condition)?;

    // Only Program-owned bounds are normalized away; all ordinary SearchLibrary validation
    // remains authoritative for this otherwise standard search.
    validate_resultless_direct_search(effect, context)
}

fn is_resultless_conditional_top_search(effect: &SpellEffectKind) -> bool {
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
    let exact_conditional_destination = conditional_destination.as_ref().is_some_and(|actual| {
        actual.destination == (SearchDestination::Battlefield { tapped: true })
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

fn validate_program_draw_recipient(
    recipient: PlayerRecipient,
    context: EffectContext,
) -> Result<(), String> {
    if recipient == PlayerRecipient::EachOtherPlayerThanAffectedPlayer
        && context == EffectContext::Ability
    {
        return Ok(());
    }
    validate_program_recipient(recipient, context, "Draw recipient")
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

fn exact_trigger_object_linked_exile_body(body: &AuthoredBlock) -> bool {
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
        SpellEffectKind::Exile {
            subject: EffectSubject::TriggerObject,
            linked_exile_id: Some(_),
        }
    )
}

fn exact_mimic_vat_imprint_receipt_body(owner: &ProgramOwnerId, body: &AuthoredBlock) -> bool {
    let exact_owner = matches!(
        owner,
        ProgramOwnerId::Ability(definition)
            if definition.card_id == "mimic_vat"
                && definition.face_id.as_str() == "mimic_vat"
                && definition.ability_path.len() == 1
                && definition.ability_path[0].as_str() == "imprint"
    );
    if !exact_owner || !body.yields.is_empty() {
        return false;
    }
    let [exile_instruction, return_instruction] = body.steps.as_slice() else {
        return false;
    };
    if exile_instruction.bind.len() != 1
        || exile_instruction.input.is_some()
        || !return_instruction.bind.is_empty()
        || return_instruction.input.as_deref() != Some(exile_instruction.bind[0].as_str())
    {
        return false;
    }
    let exact_exile = matches!(
        &exile_instruction.kind,
        InstructionKind::Perform(effect)
            if matches!(
                effect.as_ref(),
                SpellEffectKind::Exile {
                    subject: EffectSubject::TriggerObject,
                    linked_exile_id: Some(linked_exile_id),
                } if linked_exile_id.as_str() == "imprinted_card"
            )
    );
    let exact_consumer = matches!(
        &return_instruction.kind,
        InstructionKind::Perform(effect)
            if matches!(
                effect.as_ref(),
                SpellEffectKind::ReturnOtherLinkedExiledCards {
                    producer_ability_id,
                    linked_exile_id,
                } if producer_ability_id.as_str() == "imprint"
                    && linked_exile_id.as_str() == "imprinted_card"
            )
    );
    exact_exile && exact_consumer
}

fn is_mimic_vat_create_copy_owner(owner: &ProgramOwnerId) -> bool {
    matches!(
        owner,
        ProgramOwnerId::Ability(definition)
            if definition.card_id == "mimic_vat"
                && definition.face_id.as_str() == "mimic_vat"
                && definition.ability_path.len() == 1
                && definition.ability_path[0].as_str() == "create_copy"
    )
}

fn is_koya_outer_program_owner(owner: &ProgramOwnerId) -> bool {
    matches!(
        owner,
        ProgramOwnerId::Ability(definition)
            if definition.card_id == "koya,_death_from_above"
                && definition.face_id.as_str() == "koya_death_from_above"
                && definition.ability_path.len() == 1
                && definition.ability_path[0].as_str() == "triggered_01"
    )
}

fn is_exact_koya_other_creature_filter(filter: &TargetFilter) -> bool {
    filter
        == &TargetFilter {
            kind: TargetKind::Creature,
            excluded_objects: vec![super::TargetObjectExclusion::Source],
            ..TargetFilter::default()
        }
}

fn koya_outer_exile_target_group(
    instruction_id: &str,
    references: &[ProgramTargetReference],
    targeting: Option<&TargetingDef>,
) -> Result<(u32, u32), String> {
    let invalid = || {
        "Koya triggered_01 Exile requires its optional other-Creature target group 0".to_string()
    };
    let reference = single_instruction_target_binding(
        instruction_id,
        references,
        ProgramTargetRole::Subject,
        "Koya triggered_01 Exile",
    )
    .map_err(|_| invalid())?;
    let Some(group) = targeting.and_then(|targeting| targeting.groups.first()) else {
        return Err(invalid());
    };
    if reference.target_group_index != 0
        || group.chooser != super::TargetChooser::Controller
        || group.min != 0
        || group.max != 1
        || !group.effect_indices.is_empty()
        || !group.distinct_from.is_empty()
        || group.same_graveyard
        || group.cast_cost_expansion.is_some()
    {
        return Err(invalid());
    }
    Ok((0, 1))
}

fn is_mosswort_bridge_activated_02(owner: &ProgramOwnerId) -> bool {
    matches!(
        owner,
        ProgramOwnerId::Ability(definition)
            if definition.card_id == "mosswort_bridge"
                && definition.face_id.as_str() == "mosswort_bridge"
                && definition.ability_path.len() == 1
                && definition.ability_path[0].as_str() == "activated_02"
    )
}

fn validate_terminal_linked_play_profile(program: &CompiledProgram) -> Result<(), String> {
    if !compiled_block_contains_linked_play(&program.body) {
        return Ok(());
    }

    let invalid = || {
        "PlayLinkedExiledCard in a resolution program requires the exact terminal Mosswort Bridge activated_02 Choose profile"
            .to_string()
    };
    if !is_mosswort_bridge_activated_02(&program.owner)
        || program.context != EffectContext::Ability
        || !program.required_inputs.is_empty()
        || !program.target_bindings.is_empty()
        || !program.group_inputs.is_empty()
        || !program.slots.is_empty()
        || !program.body.yields.is_empty()
    {
        return Err(invalid());
    }

    let [choose] = program.body.steps.as_slice() else {
        return Err(invalid());
    };
    if choose.input.is_some() || !choose.outputs.is_empty() {
        return Err(invalid());
    }
    let CompiledInstructionKind::Choose {
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choose.kind
    else {
        return Err(invalid());
    };
    if *chooser != PlayerRecipient::Controller
        || !*optional
        || *otherwise
            != (CompiledBlock {
                steps: Vec::new(),
                yields: Vec::new(),
            })
        || branches.len() != 1
    {
        return Err(invalid());
    }

    let branch = &branches[0];
    if branch.cost != ResolutionCost::None
        || branch.presentation != AbilityPresentation::Fallback
        || !is_exact_mosswort_total_power_condition(&branch.requirement)
        || !branch.body.yields.is_empty()
        || branch.body.steps.len() != 1
    {
        return Err(invalid());
    }
    let play = &branch.body.steps[0];
    if play.input.is_some() || !play.outputs.is_empty() {
        return Err(invalid());
    }
    if !matches!(
        &play.kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(
                effect.as_ref(),
                SpellEffectKind::PlayLinkedExiledCard {
                    producer_ability_id,
                    linked_exile_id,
                    filter: None,
                    play_kind: super::LinkedExiledCardPlayKind::Play,
                    return_source_to_owner_hand_if_cast: false,
                } if producer_ability_id.as_str() == "hideaway"
                    && linked_exile_id.as_str() == "exiled_cards"
            )
    ) {
        return Err(invalid());
    }
    Ok(())
}

fn compiled_block_contains_linked_play(block: &CompiledBlock) -> bool {
    block
        .steps
        .iter()
        .any(|instruction| match &instruction.kind {
            CompiledInstructionKind::Perform(effect) => {
                matches!(
                    effect.as_ref(),
                    SpellEffectKind::PlayLinkedExiledCard { .. }
                )
            }
            CompiledInstructionKind::If { yes, no, .. } => {
                compiled_block_contains_linked_play(yes) || compiled_block_contains_linked_play(no)
            }
            CompiledInstructionKind::Choose {
                branches,
                otherwise,
                ..
            } => {
                branches
                    .iter()
                    .any(|branch| compiled_block_contains_linked_play(&branch.body))
                    || compiled_block_contains_linked_play(otherwise)
            }
            CompiledInstructionKind::ImportCastPayment => false,
        })
}

fn is_exact_mosswort_total_power_condition(requirement: &CompiledChooseRequirement) -> bool {
    matches!(
        requirement,
        CompiledChooseRequirement::GameCondition(condition)
            if matches!(
                condition,
                GameCondition::BattlefieldAggregate {
                    filter,
                    aggregate: super::BattlefieldAggregate::TotalPower,
                    min: Some(10),
                    max: None,
                    exclude_observed_object: false,
                } if validate_program_total_power_filter(filter).is_ok()
            )
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
        GameCondition::BattlefieldAggregate {
            filter,
            aggregate: super::BattlefieldAggregate::MaximumPower,
            min: Some(min),
            max: None,
            exclude_observed_object: false,
        } => validate_program_maximum_power_filter(filter, *min)?,
        GameCondition::BattlefieldAggregate {
            filter,
            aggregate: super::BattlefieldAggregate::TotalPower,
            min: Some(10),
            max: None,
            exclude_observed_object: false,
        } => validate_program_total_power_filter(filter)?,
        GameCondition::AllOf(branches) | GameCondition::AnyOf(branches) => {
            for branch in branches {
                validate_program_condition(branch)?;
            }
        }
        _ => return Err(PROGRAM_CONDITION_UNSUPPORTED.into()),
    }
    condition.validate_live()
}

fn is_exact_delirium_program_condition(condition: &GameCondition) -> bool {
    matches!(
        condition,
        GameCondition::GraveyardAggregate {
            owners: RelativePlayerSet::Controller,
            aggregate: super::GraveyardAggregate::DistinctCardTypes,
            filter: None,
            min: Some(4),
            max: None,
        }
    )
}

/// Ability Programs may inspect only a live source permanent and its own bounded counter total.
/// Restricting every composite leaf to this family keeps these conditions independent of target,
/// event, and prior-result context.
fn is_ability_source_condition_family(condition: &GameCondition) -> bool {
    match condition {
        GameCondition::SourceCounterCount { .. } => true,
        GameCondition::ObjectMatches {
            object: super::ConditionObjectRef::Source,
            filter,
        } => {
            filter.as_ref()
                == &TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    ..TargetFilter::default()
                }
        }
        GameCondition::AllOf(branches) | GameCondition::AnyOf(branches) => {
            !branches.is_empty() && branches.iter().all(is_ability_source_condition_family)
        }
        _ => false,
    }
}

/// Trade Route Envoy's mandatory Choose branch is available only when its controller controls at
/// least one creature with any counter. Delta Bloodflies uses the same live predicate on its
/// intervening-if; this narrow Choose-only admission relies on Program's draw-selectability gate
/// and does not widen generic Program `If` conditions.
fn is_exact_ability_choose_countered_creature_count(condition: &GameCondition) -> bool {
    matches!(
        condition,
        GameCondition::BattlefieldCreatureCount {
            filter,
            min: Some(1),
            max: None,
        } if filter.controllers == RelativePlayerSet::Controller
            && filter.subtype.is_none()
            && filter.required_keywords.is_empty()
            && filter.tapped.is_none()
            && filter.requires_any_counter
            && filter.required_counter.is_none()
            && !filter.exclude_source
    )
}

const PROGRAM_CONDITION_UNSUPPORTED: &str = concat!(
    "If condition uses context outside controller hand, library, CardCount graveyard, or ",
    "Controller battlefield Count state supported by spell_program; ProgramIf also admits ",
    "only positive-min Controller Creature MaximumPower without max, extra filter fields, ",
    "or observed-object exclusion, exact Controller Creature TotalPower min 10, and the ",
    "exact Controller four-type Delirium profile",
);

fn validate_program_maximum_power_filter(
    filter: &super::BattlefieldPermanentFilter,
    min: u32,
) -> Result<(), String> {
    if min == 0
        || filter.controllers != RelativePlayerSet::Controller
        || filter.card_type != Some(CardTypeFilter::Creature)
        || !filter.required_supertypes.is_empty()
        || filter.token.is_some()
        || filter.any_of.is_some()
        || filter.color.is_some()
        || filter.name.is_some()
        || !filter.required_subtypes.is_empty()
        || filter.exclude_source
    {
        return Err(PROGRAM_CONDITION_UNSUPPORTED.into());
    }
    Ok(())
}

fn validate_program_total_power_filter(
    filter: &super::BattlefieldPermanentFilter,
) -> Result<(), String> {
    if filter
        != &(super::BattlefieldPermanentFilter {
            required_supertypes: Vec::new(),
            token: None,
            any_of: None,
            controllers: RelativePlayerSet::Controller,
            card_type: Some(CardTypeFilter::Creature),
            color: None,
            name: None,
            required_subtypes: Vec::new(),
            exclude_source: false,
        })
    {
        return Err(PROGRAM_CONDITION_UNSUPPORTED.into());
    }
    Ok(())
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

fn validate_program_tap_permanent_cost_filter(filter: &TargetFilter) -> Result<(), String> {
    if filter.any_of.is_some() {
        return Err("filter cannot use any_of".into());
    }
    if filter.controller != super::TargetController::You {
        return Err("filter requires controller You".into());
    }
    if !matches!(filter.kind, TargetKind::Creature | TargetKind::AnyPermanent) {
        return Err("filter kind must be Creature or AnyPermanent".into());
    }
    if !filter.excluded_objects.is_empty() {
        return Err("filter cannot use excluded_objects".into());
    }
    filter.validate_characteristic_constraints()
}

fn program_sacrifice_has_any_of_branch_exclusions(filter: &TargetFilter) -> bool {
    let Some(branches) = &filter.any_of else {
        return false;
    };

    branches.iter().any(|branch| {
        !branch.excluded_objects.is_empty()
            || program_sacrifice_has_any_of_branch_exclusions(branch)
    })
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

fn validate_program_create_tokens_amount(
    amount: &Amount,
    recipient: &PlayerRecipient,
) -> Result<(), String> {
    validate_program_amount(amount)?;
    match amount {
        Amount::Fixed(_) => Ok(()),
        Amount::Count(super::CountExpression::GraveyardCards {
            owners: RelativePlayerSet::Controller,
            filter: Some(_),
        }) if recipient == &PlayerRecipient::Controller => Ok(()),
        _ => Err(
            "CreateTokens count supports only a fixed literal or an explicitly filtered Controller GraveyardCards count for Controller token creation in spell_program".into(),
        ),
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

#[cfg(test)]
mod result_type_join_tests {
    use super::{join_result_types, CardResultAction, Cardinality, ResultType};

    fn cohort(action: CardResultAction, target_group_index: u32) -> ResultType {
        ResultType::TargetCardResultCohort {
            action,
            cardinality: Cardinality {
                min: 0,
                max: Some(2),
            },
            target_group_index,
        }
    }

    #[test]
    fn targeted_card_cohort_joins_preserve_action_and_group_origin() {
        let exile = cohort(CardResultAction::Exile, 0);
        assert_eq!(
            join_result_types(&exile, &exile),
            Some(exile.clone()),
            "matching Exile cohorts from the same target group can join"
        );
        assert!(
            join_result_types(&exile, &cohort(CardResultAction::Exile, 1)).is_none(),
            "cohorts from different target groups cannot join"
        );
        assert!(
            join_result_types(&exile, &cohort(CardResultAction::Destroy, 0)).is_none(),
            "different card actions cannot join even when they share a target group"
        );
    }
}
