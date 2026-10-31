use tricerules_cards::primitives::{
    Cardinality, CompiledInstructionKind, EffectContext, EffectSubject, ResultType, SpellEffectKind,
};
use tricerules_cards::ProgramOwnerId;

const RIPPLE: &str = "ripples_of_potential";

#[test]
fn ripples_has_exact_identity_and_typed_receipt_program_shape() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(registry.id_for_name("Ripples of Potential"), Some(RIPPLE));
    let card = registry
        .get(RIPPLE)
        .expect("Ripples of Potential is registered");
    assert_eq!(card.id, RIPPLE);
    assert_eq!(card.name, "Ripples of Potential");
    assert!(
        !card.commander_setup_only,
        "the full Instant definition is admitted as an ordinary card"
    );

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), RIPPLE);
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Instant"]);
    assert!(face.spell_effect.is_empty());
    assert!(face.targeting.is_none());

    let owner = ProgramOwnerId::face(RIPPLE, face.face_id.clone());
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("the exact Ripples spell face has a compiled program");
    assert_eq!(program.owner, owner);
    assert_eq!(program.context, EffectContext::Spell);
    assert!(program.required_inputs.is_empty());
    assert!(program.target_bindings.is_empty());
    assert!(program.body.yields.is_empty());
    assert_eq!(program.slots.len(), 2);
    let [proliferate, choose, phase_out] = program.body.steps.as_slice() else {
        panic!("Ripples has exactly three sequential Program leaves");
    };
    let CompiledInstructionKind::Perform(proliferate_effect) = &proliferate.kind else {
        panic!("the first step performs Proliferate");
    };
    assert_eq!(**proliferate_effect, SpellEffectKind::Proliferate);
    assert!(proliferate.input.is_none());
    let [cohort_slot] = proliferate.outputs.as_slice() else {
        panic!("Program Proliferate binds exactly one typed cohort");
    };
    assert_eq!(
        program.slot(*cohort_slot).map(|slot| &slot.result_type),
        Some(&ResultType::ProliferatedPermanentCohort {
            cardinality: Cardinality { min: 0, max: None },
        })
    );

    let CompiledInstructionKind::Perform(choose_effect) = &choose.kind else {
        panic!("the second step performs the specialized permanent choice");
    };
    assert_eq!(
        **choose_effect,
        SpellEffectKind::ChooseProliferatedPermanents
    );
    assert_eq!(choose.input, Some(*cohort_slot));
    let [selection_slot] = choose.outputs.as_slice() else {
        panic!("the choice binds exactly one typed permanent selection");
    };
    assert_eq!(
        program.slot(*selection_slot).map(|slot| &slot.result_type),
        Some(&ResultType::ProliferatedPermanentSelection {
            cardinality: Cardinality { min: 0, max: None },
        })
    );

    let CompiledInstructionKind::Perform(phase_effect) = &phase_out.kind else {
        panic!("the final step phases out the chosen permanents");
    };
    assert_eq!(
        **phase_effect,
        SpellEffectKind::PhaseOut {
            subject: EffectSubject::PreviousEffectObjects,
        }
    );
    assert_eq!(phase_out.input, Some(*selection_slot));
    assert!(phase_out.outputs.is_empty());
}
