#![allow(dead_code)] // Each integration test compiles common independently and uses only part of this helper.

use tricerules_cards::primitives::{
    CompiledInstruction, CompiledInstructionKind, CompiledProgramCondition,
    CompiledProgramResultCondition, DiscardQuantity, DiscardResultOutcome, PlayerRecipient,
    SpellEffectKind,
};
use tricerules_cards::{
    AbilityDefinitionId, AbilityId, Amount, CardFaceId, CardRegistry, ProgramOwnerId,
};

#[derive(Clone, Copy)]
pub enum AuthoredDrawDiscardOrder {
    DrawThenDiscard,
    DiscardThenDraw,
    OptionalDiscardThenDraw,
}

#[derive(Clone, Copy)]
pub struct AuthoredDrawDiscardExpectation {
    pub first_instruction: usize,
    pub trailing_instructions: usize,
    pub who: PlayerRecipient,
    pub draw_count: u32,
    pub discard_count: u32,
    pub order: AuthoredDrawDiscardOrder,
}

pub fn assert_face_draw_discard_steps<'a>(
    registry: &'a CardRegistry,
    card_id: &str,
    face_id: &CardFaceId,
    expected: AuthoredDrawDiscardExpectation,
) -> &'a tricerules_cards::primitives::CompiledProgram {
    assert_draw_discard_steps(
        registry,
        ProgramOwnerId::face(card_id, face_id.clone()),
        expected,
    )
}

pub fn assert_ability_draw_discard_steps<'a>(
    registry: &'a CardRegistry,
    card_id: &str,
    face_id: &CardFaceId,
    ability_id: &AbilityId,
    expected: AuthoredDrawDiscardExpectation,
) -> &'a tricerules_cards::primitives::CompiledProgram {
    assert_draw_discard_steps(
        registry,
        ProgramOwnerId::ability(AbilityDefinitionId {
            card_id: card_id.to_owned(),
            face_id: face_id.clone(),
            ability_path: vec![ability_id.clone()],
        }),
        expected,
    )
}

pub fn assert_draw_discard_steps(
    registry: &CardRegistry,
    owner: ProgramOwnerId,
    expected: AuthoredDrawDiscardExpectation,
) -> &tricerules_cards::primitives::CompiledProgram {
    let program = registry
        .compiled_program_for_owner(&owner)
        .unwrap_or_else(|| panic!("missing compiled program for {owner:?}"));
    assert_eq!(program.owner, owner);
    assert!(program.body.yields.is_empty());
    if expected.first_instruction == 0 {
        assert!(program.target_bindings.is_empty());
    }

    let draw = SpellEffectKind::Draw {
        who: expected.who,
        count: Amount::Fixed(expected.draw_count),
    };
    let discard = |quantity| SpellEffectKind::Discard {
        who: expected.who,
        quantity,
    };
    let steps = &program.body.steps;
    let expected_step_count = expected.first_instruction + 2 + expected.trailing_instructions;
    match expected.order {
        AuthoredDrawDiscardOrder::DrawThenDiscard => {
            assert_eq!(steps.len(), expected_step_count);
            assert_perform(&steps[expected.first_instruction], draw);
            assert_perform(
                &steps[expected.first_instruction + 1],
                discard(DiscardQuantity::Exact(expected.discard_count)),
            );
        }
        AuthoredDrawDiscardOrder::DiscardThenDraw => {
            assert_eq!(steps.len(), expected_step_count);
            assert_perform(
                &steps[expected.first_instruction],
                discard(DiscardQuantity::Exact(expected.discard_count)),
            );
            assert_perform(&steps[expected.first_instruction + 1], draw);
        }
        AuthoredDrawDiscardOrder::OptionalDiscardThenDraw => {
            assert_eq!(steps.len(), expected_step_count);
            let discard_step = &steps[expected.first_instruction];
            assert_perform(
                discard_step,
                discard(DiscardQuantity::OptionalExact(expected.discard_count)),
            );
            let [result] = discard_step.outputs.as_slice() else {
                panic!("optional discard must produce the result used by its branch");
            };
            let CompiledInstructionKind::If { condition, yes, no } =
                &steps[expected.first_instruction + 1].kind
            else {
                panic!("draw follows the optional discard only through a result branch");
            };
            assert!(matches!(
                condition.as_ref(),
                CompiledProgramCondition::Result(
                    CompiledProgramResultCondition::DiscardOutcome {
                        result: branch_result,
                        outcome: DiscardResultOutcome::ChosenToPay,
                    }
                ) if branch_result == result
            ));
            assert!(yes.yields.is_empty());
            assert!(no.steps.is_empty() && no.yields.is_empty());
            let [draw_step] = yes.steps.as_slice() else {
                panic!("choosing the optional discard leads to exactly one draw");
            };
            assert_perform(draw_step, draw);
        }
    }
    program
}

fn assert_perform(step: &CompiledInstruction, expected: SpellEffectKind) {
    assert!(matches!(
        &step.kind,
        CompiledInstructionKind::Perform(effect) if **effect == expected
    ));
}
