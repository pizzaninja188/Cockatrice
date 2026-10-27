use tricerules_cards::primitives::{
    CompiledInstructionKind, PlayerRecipient, ProgramRequiredInput, ResolutionCost,
    SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{AbilityDefinitionId, AbilityId, CardFaceId, ProgramOwnerId};

const RINGS: &str = include_str!("../data/rings_of_brighthearth.ron");

#[test]
fn rings_has_the_exact_nonmana_trigger_and_optional_two_mana_copy_program() {
    let registry = tricerules_cards::registry::global();
    let card = registry.get("rings_of_brighthearth").unwrap();
    let face = card.primary_face();
    assert_eq!(face.name, "Rings of Brighthearth");
    assert_eq!(face.mana_cost.to_string(), "{3}");
    assert_eq!(face.types, ["Artifact"]);
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Rings has one triggered ability");
    };
    assert_eq!(
        trigger.trigger,
        TriggerCondition::WheneverControllerActivatesNonManaAbility
    );
    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: card.id.clone(),
        face_id: CardFaceId::new("rings_of_brighthearth").unwrap(),
        ability_path: vec![AbilityId::new("triggered_01").unwrap()],
    });
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("compiled Rings trigger");
    assert!(program
        .required_inputs
        .contains(&ProgramRequiredInput::TriggerActivatedStackObject));
    let [step] = program.body.steps.as_slice() else {
        panic!("one optional choice");
    };
    let CompiledInstructionKind::Choose {
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &step.kind
    else {
        panic!("the optional payment is a Program choice");
    };
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("one payment branch");
    };
    assert!(matches!(&branch.cost, ResolutionCost::Mana(cost) if cost.to_string() == "{2}"));
    let [copy] = branch.body.steps.as_slice() else {
        panic!("one copy operation");
    };
    assert!(
        matches!(&copy.kind, CompiledInstructionKind::Perform(effect)
        if matches!(effect.as_ref(), SpellEffectKind::CopyTriggeringActivatedAbility))
    );
    assert!(copy.input.is_none() && copy.outputs.is_empty());
}

#[test]
fn rings_copy_program_is_admitted_only_with_the_supplying_trigger() {
    let wrong_trigger = RINGS.replace(
        "WheneverControllerActivatesNonManaAbility",
        "AtBeginningOfEndStep()",
    );
    let error = tricerules_cards::CardRegistry::from_chunks_and_tokens(&[&wrong_trigger], &[])
        .expect_err("an unrelated trigger cannot supply the observed activation");
    assert!(
        error.to_string().contains("TriggerActivatedStackObject"),
        "{error}"
    );

    let legacy = r#"(
        id: "bad_rings_legacy", name: "Bad Rings Legacy", face_id: "bad_rings_legacy",
        types: ["Artifact"], triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
            trigger: WheneverControllerActivatesNonManaAbility,
            effect: [CopyTriggeringActivatedAbility],
        )],
    )"#;
    let error = tricerules_cards::CardRegistry::from_chunks_and_tokens(&[legacy], &[])
        .expect_err("the legacy effect path cannot bypass Program input admission");
    assert!(
        error
            .to_string()
            .contains("requires an authored trigger Program"),
        "{error}"
    );

    let wrong_owner = r#"(
        id: "bad_rings_spell", name: "Bad Rings Spell", face_id: "bad_rings_spell",
        types: ["Sorcery"], spell_program: Some((body: (steps: [
            (id: "copy", kind: Perform(CopyTriggeringActivatedAbility)),
        ]))),
    )"#;
    let error = tricerules_cards::CardRegistry::from_chunks_and_tokens(&[wrong_owner], &[])
        .expect_err("a spell cannot supply a trigger's activation receipt");
    assert!(
        error.to_string().contains("ability-owned trigger Program"),
        "{error}"
    );
}
