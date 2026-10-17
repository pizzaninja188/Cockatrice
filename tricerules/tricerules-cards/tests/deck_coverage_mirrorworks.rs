use tricerules_cards::primitives::{
    CastTriggerPlayer, CompiledInstructionKind, InstructionKind, PermanentTypeFilter,
    PlayerRecipient, ProgramRequiredInput, ResolutionCost, TokenCopySource, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, Layout, ManaCost};

#[test]
fn mirrorworks_is_registered_as_a_complete_card() {
    let card = tricerules_cards::registry::global()
        .get("mirrorworks")
        .expect("Mirrorworks needs a complete definition");
    assert_eq!(card.name, "Mirrorworks");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "mirrorworks");
    assert_eq!(face.mana_cost, ManaCost::parse("{5}").unwrap());
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.keywords.is_empty());

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Mirrorworks has exactly one triggered ability");
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert!(matches!(
        &trigger.presentation,
        AbilityPresentation::OracleLines(lines) if lines == &[1]
    ));
    match &trigger.trigger {
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller,
            filter,
            creature_filter,
        } => {
            assert_eq!(*controller, CastTriggerPlayer::Controller);
            assert_eq!(filter.permanent_type, Some(PermanentTypeFilter::Artifact));
            assert_eq!(filter.token, Some(false));
            assert!(filter.exclude_source);
            assert!(creature_filter.is_none());
        }
        other => panic!("unexpected Mirrorworks trigger: {other:?}"),
    }
    assert!(trigger.effect.is_empty());
    let program = trigger
        .program
        .as_ref()
        .expect("the exact entry ability owns the authored copy program");
    assert!(program.target_bindings.is_empty());
    let [choose] = program.body.steps.as_slice() else {
        panic!("the one optional choice is the complete Mirrorworks program");
    };
    let InstructionKind::Choose {
        choice,
        chooser,
        optional,
        branches,
        otherwise,
    } = &choose.kind
    else {
        panic!("Mirrorworks begins with its optional payment choice");
    };
    assert_eq!(choice.as_str(), "pay_to_copy");
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("Mirrorworks has one paid copy branch");
    };
    assert_eq!(branch.branch_id.as_str(), "pay_to_copy");
    assert!(matches!(
        &branch.presentation,
        AbilityPresentation::OracleLines(lines) if lines == &[1]
    ));
    assert!(matches!(
        &branch.cost,
        ResolutionCost::Mana(cost) if cost == &ManaCost::parse("{2}").unwrap()
    ));
    assert_eq!(branch.requirement, Default::default());
    assert!(matches!(
        branch.body.steps.as_slice(),
        [step] if matches!(
            &step.kind,
            InstructionKind::Perform(effect)
                if matches!(effect.as_ref(), tricerules_cards::SpellEffectKind::CreateTokenCopies {
                    count: tricerules_cards::Amount::Fixed(1),
                    source: TokenCopySource::TriggerObject,
                })
        )
    ));
    assert!(branch.body.yields.is_empty());
}

#[test]
fn mirrorworks_trigger_program_registers_its_exact_observed_object_input() {
    let owner = tricerules_cards::ProgramOwnerId::ability(tricerules_cards::AbilityDefinitionId {
        card_id: "mirrorworks".into(),
        face_id: tricerules_cards::CardFaceId::new("mirrorworks").unwrap(),
        ability_path: vec![tricerules_cards::AbilityId::new("triggered_01").unwrap()],
    });
    let compiled = tricerules_cards::registry::global()
        .compiled_program_for_owner(&owner)
        .expect("Mirrorworks triggered_01 compiles its authored observed-object program");
    assert_eq!(
        compiled.required_inputs,
        vec![ProgramRequiredInput::TriggerObservedObject]
    );
    assert!(compiled.target_bindings.is_empty());
    assert!(compiled.slots.is_empty());
    assert!(matches!(
        &compiled.body.steps[0].kind,
        CompiledInstructionKind::Choose { branches, otherwise, .. }
            if otherwise.steps.is_empty()
                && matches!(
                    branches.as_slice(),
                    [branch] if matches!(
                        branch.body.steps.as_slice(),
                        [instruction] if instruction.outputs.is_empty()
                            && matches!(
                                &instruction.kind,
                                CompiledInstructionKind::Perform(effect)
                                    if matches!(effect.as_ref(), tricerules_cards::SpellEffectKind::CreateTokenCopies {
                                        source: TokenCopySource::TriggerObject,
                                        ..
                                    })
                            )
                    )
                )
    ));
}
