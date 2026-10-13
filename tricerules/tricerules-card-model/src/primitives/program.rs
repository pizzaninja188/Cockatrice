//! Finite, typed resolution programs for cards whose ordered actions need explicit results.
//!
//! Programs reuse the ordinary effect vocabulary for leaf actions. Their compiler gives results
//! stable slots and requires every branch to declare the values it yields to its parent.
//! The initial runner is deliberately narrower than that vocabulary: spell programs are
//! targetless, controller-scoped instant/sorcery sequences. Draw and private hand-choice amounts
//! are fixed literals, `If` accepts only controller-library/hand-count conditions, and permanent
//! choices cannot use event-bound filters or cross-object constraints. Widening these rules needs
//! a separately typed context/result contract.

use super::{Amount, EffectContext, GameCondition, PlayerRecipient, SpellEffectKind, TargetFilter};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardProgram {
    pub body: AuthoredBlock,
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
    DiscardReceipt {
        who: PlayerRecipient,
        cardinality: Cardinality,
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
    pub kind: CompiledInstructionKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompiledInstructionKind {
    Perform(Box<EffectOp>),
    If {
        condition: Box<GameCondition>,
        yes: CompiledBlock,
        no: CompiledBlock,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledProgram {
    pub card_id: String,
    pub face_id: String,
    pub body: CompiledBlock,
    pub slots: Vec<CompiledSlot>,
}

struct Compiler {
    slots: Vec<CompiledSlot>,
    instruction_ids: HashSet<String>,
}

impl CompiledProgram {
    pub fn compile(
        card_id: impl Into<String>,
        face_id: impl Into<String>,
        program: &CardProgram,
    ) -> Result<Self, String> {
        if program.body.steps.is_empty() {
            return Err("program body must contain at least one instruction".into());
        }
        let mut compiler = Compiler {
            slots: Vec::new(),
            instruction_ids: HashSet::new(),
        };
        let (body, _) = compiler.compile_block(&program.body, &HashMap::new())?;
        Ok(Self {
            card_id: card_id.into(),
            face_id: face_id.into(),
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
                    (
                        input,
                        validate_and_result_types(effect)?,
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
                    validate_program_condition(condition).map_err(|reason| {
                        format!("If instruction '{}': {reason}", instruction.id)
                    })?;
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
                            condition: condition.clone(),
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
            ResultType::DiscardReceipt {
                who: a,
                cardinality: ac,
            },
            ResultType::DiscardReceipt {
                who: b,
                cardinality: bc,
            },
        ) if a == b => ResultType::DiscardReceipt {
            who: *a,
            cardinality: union(*ac, *bc),
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

fn validate_and_result_types(effect: &SpellEffectKind) -> Result<Vec<ResultType>, String> {
    match effect {
        SpellEffectKind::Draw { who, count } => {
            effect.validate(EffectContext::Spell)?;
            validate_program_controller(*who, "Draw recipient")?;
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
            effect.validate(EffectContext::Spell)?;
            validate_program_controller(*who, "Discard recipient")?;
            Ok(vec![ResultType::DiscardReceipt {
                who: *who,
                cardinality: Cardinality {
                    min: 0,
                    max: discard_upper_bound(quantity),
                },
            }])
        }
        SpellEffectKind::ChoosePermanents {
            chooser,
            filter,
            max,
            constraints,
            ..
        } => {
            effect.validate(EffectContext::Spell)?;
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
            effect.validate(EffectContext::Spell)?;
            Ok(Vec::new())
        }
        SpellEffectKind::SelectHandCards {
            count,
            ordered,
            prompt,
        } => {
            count.validate_effect(EffectContext::Spell)?;
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
        _ => return Err("If condition uses context outside controller hand and library state supported by spell_program".into()),
    }
    condition.validate_live()
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
        super::DiscardQuantity::Exact(count) | super::DiscardQuantity::UpTo(count) => Some(*count),
        super::DiscardQuantity::AnyNumber | super::DiscardQuantity::All => None,
        super::DiscardQuantity::UnlessOne { count, .. } => Some(*count),
    }
}
