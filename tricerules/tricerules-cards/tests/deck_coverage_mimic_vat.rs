use tricerules_cards::primitives::{
    Amount, EffectContext, EffectSubject, InstructionKind, Keyword, PlayerRecipient, ResultType,
    SpellEffectKind, TokenCopySource, TriggerCondition,
};
use tricerules_cards::{
    AbilityDefinitionId, AbilityId, AbilityLinkId, CardFaceId, CardProgram, CardRegistry,
    CompiledProgram, ProgramOwnerId,
};

const MIMIC_VAT_RON: &str = include_str!("../data/mimic_vat.ron");

fn actual_mimic_vat_registry() -> CardRegistry {
    CardRegistry::from_chunks_and_tokens(&[MIMIC_VAT_RON], &[])
        .unwrap_or_else(|error| panic!("actual Mimic Vat RON should compile: {error}"))
}

fn actual_create_copy_program() -> CardProgram {
    let registry = actual_mimic_vat_registry();
    let card = registry
        .get("mimic_vat")
        .expect("the actual Mimic Vat definition is registered");
    let [ability] = card.primary_face().activated_abilities.as_slice() else {
        panic!("Mimic Vat has one activated ability");
    };
    ability
        .program
        .clone()
        .expect("create_copy is authored as a Program")
}

fn mimic_vat_create_copy_owner() -> ProgramOwnerId {
    ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "mimic_vat".into(),
        face_id: CardFaceId::new("mimic_vat").unwrap(),
        ability_path: vec![AbilityId::new("create_copy").unwrap()],
    })
}

fn compile_mimic_vat_create_copy(program: &CardProgram) -> Result<CompiledProgram, String> {
    CompiledProgram::compile(
        mimic_vat_create_copy_owner(),
        EffectContext::Ability,
        program,
    )
}

fn copy_source_mut(program: &mut CardProgram) -> &mut TokenCopySource {
    let first = program
        .body
        .steps
        .first_mut()
        .expect("create_copy begins with its copy producer");
    let InstructionKind::Perform(effect) = &mut first.kind else {
        panic!("the first create_copy instruction is a Perform leaf");
    };
    let SpellEffectKind::CreateTokenCopies { source, .. } = effect.as_mut() else {
        panic!("the first create_copy leaf is CreateTokenCopies");
    };
    source
}

fn copy_count_mut(program: &mut CardProgram) -> &mut Amount {
    let first = program
        .body
        .steps
        .first_mut()
        .expect("create_copy begins with its copy producer");
    let InstructionKind::Perform(effect) = &mut first.kind else {
        panic!("the first create_copy instruction is a Perform leaf");
    };
    let SpellEffectKind::CreateTokenCopies { count, .. } = effect.as_mut() else {
        panic!("the first create_copy leaf is CreateTokenCopies");
    };
    count
}

fn assert_linked_copy_profile_rejected(
    program: &CardProgram,
    owner: ProgramOwnerId,
    context: EffectContext,
) {
    let error = CompiledProgram::compile(owner, context, program)
        .expect_err("only the exact Mimic Vat linked-copy profile compiles");
    assert!(
        error.contains("LinkedExile token copies require Mimic Vat's exact create_copy ability")
            || error
                .contains("linked-exile token copies require an activated or triggered ability"),
        "unexpected rejection: {error}"
    );
}

#[test]
fn mimic_vat_create_copy_program_compiles_from_the_actual_card_definition() {
    let registry = actual_mimic_vat_registry();
    let card = registry
        .get("mimic_vat")
        .expect("the actual Mimic Vat definition is registered");
    let face = card.primary_face();
    let [imprint] = face.triggered_abilities.as_slice() else {
        panic!("the existing imprint ability remains on Mimic Vat");
    };
    assert_eq!(imprint.ability_id.as_str(), "imprint");
    assert!(imprint.effect.is_empty());
    assert!(imprint.program.is_some());

    let [copy_ability] = face.activated_abilities.as_slice() else {
        panic!("Mimic Vat has one activated ability");
    };
    assert_eq!(copy_ability.ability_id.as_str(), "create_copy");
    assert!(copy_ability.effect.is_empty());
    let authored = copy_ability
        .program
        .as_ref()
        .expect("create_copy is authored as a Program");
    let [copy_step, haste_step, delayed_step] = authored.body.steps.as_slice() else {
        panic!("the Program copies, grants Haste, then schedules exile");
    };
    assert_eq!(copy_step.id, "copy_entry");
    assert_eq!(copy_step.bind.len(), 1);
    assert_eq!(copy_step.bind[0], "copies");
    assert!(copy_step.input.is_none());
    assert_eq!(haste_step.input.as_deref(), Some("copies"));
    assert_eq!(delayed_step.input.as_deref(), Some("copies"));

    assert!(matches!(
        &copy_step.kind,
        InstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::CreateTokenCopies {
                count: Amount::Fixed(1),
                source: TokenCopySource::LinkedExile { producer_ability_id, linked_exile_id },
            } if producer_ability_id.as_str() == "imprint"
                && linked_exile_id.as_str() == "imprinted_card")
    ));
    assert!(matches!(
        &haste_step.kind,
        InstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::ApplyPermanentModifier {
                subject: EffectSubject::PreviousEffectObjects,
                modifier: tricerules_cards::ResolvingPermanentModifier::GrantKeywords(keywords),
                duration: tricerules_cards::ResolvingEffectDuration::Indefinite,
            } if keywords.as_slice() == [Keyword::Haste])
    ));
    assert!(matches!(
        &delayed_step.kind,
        InstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::CreateDelayedTrigger {
                subject: Some(EffectSubject::PreviousEffectObjects),
                affected_player: None,
                ability,
            } if matches!(ability.trigger, TriggerCondition::AtBeginningOfNextEndStep)
                && matches!(ability.effect.as_slice(), [SpellEffectKind::ExileObservedObjects])
                && ability.modal.is_none()
                && ability.targeting.is_none()
                && !ability.may)
    ));

    let compiled = CompiledProgram::compile(
        mimic_vat_create_copy_owner(),
        EffectContext::Ability,
        authored,
    )
    .expect("the actual create_copy owner compiles its typed batch");
    let [copy_step, haste_step, delayed_step] = compiled.body.steps.as_slice() else {
        panic!("the compiled Program preserves the three authored steps");
    };
    let [batch_slot] = copy_step.outputs.as_slice() else {
        panic!("CreateTokenCopies publishes one typed batch slot");
    };
    assert!(matches!(
        compiled.slot(*batch_slot).map(|slot| &slot.result_type),
        Some(ResultType::TokenCopyBatch)
    ));
    assert_eq!(haste_step.input, Some(*batch_slot));
    assert_eq!(delayed_step.input, Some(*batch_slot));
}

#[test]
fn mimic_vat_linked_copy_admission_rejects_near_miss_profiles() {
    let actual = actual_create_copy_program();

    let mut wrong_producer = actual.clone();
    let TokenCopySource::LinkedExile {
        producer_ability_id,
        ..
    } = copy_source_mut(&mut wrong_producer)
    else {
        panic!("the actual source is LinkedExile");
    };
    *producer_ability_id = AbilityId::new("other_producer").unwrap();
    assert_linked_copy_profile_rejected(
        &wrong_producer,
        mimic_vat_create_copy_owner(),
        EffectContext::Ability,
    );

    let mut wrong_link = actual.clone();
    let TokenCopySource::LinkedExile {
        linked_exile_id, ..
    } = copy_source_mut(&mut wrong_link)
    else {
        panic!("the actual source is LinkedExile");
    };
    *linked_exile_id = AbilityLinkId::new("other_link").unwrap();
    assert_linked_copy_profile_rejected(
        &wrong_link,
        mimic_vat_create_copy_owner(),
        EffectContext::Ability,
    );

    let mut wrong_count = actual.clone();
    *copy_count_mut(&mut wrong_count) = Amount::Fixed(2);
    assert_linked_copy_profile_rejected(
        &wrong_count,
        mimic_vat_create_copy_owner(),
        EffectContext::Ability,
    );

    let other_owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "other_card".into(),
        face_id: CardFaceId::new("other_card").unwrap(),
        ability_path: vec![AbilityId::new("create_copy").unwrap()],
    });
    assert_linked_copy_profile_rejected(&actual, other_owner, EffectContext::Ability);

    let spell_owner = ProgramOwnerId::face("mimic_vat", CardFaceId::new("mimic_vat").unwrap());
    assert_linked_copy_profile_rejected(&actual, spell_owner, EffectContext::Spell);
}

#[test]
fn mimic_vat_typed_copy_batch_checks_consumer_input_and_result_type() {
    let actual = actual_create_copy_program();

    let mut missing_input = actual.clone();
    missing_input.body.steps[1].input = Some("missing_batch".into());
    let error = compile_mimic_vat_create_copy(&missing_input)
        .expect_err("a Haste consumer must name a bound batch");
    assert!(
        error.contains("input result 'missing_batch' is not in scope"),
        "unexpected missing-input error: {error}"
    );

    let mut wrong_type = actual;
    let InstructionKind::Perform(copy_effect) = &mut wrong_type.body.steps[0].kind else {
        panic!("the copy step is a Perform leaf");
    };
    **copy_effect = SpellEffectKind::CreateTokens {
        token: "ally_w_1_1".into(),
        count: Amount::Fixed(1),
        who: PlayerRecipient::Controller,
        tapped: false,
        sacrifice_timing: None,
    };
    let kyoshi_owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "kyoshi_battle_fan".into(),
        face_id: CardFaceId::new("kyoshi_battle_fan").unwrap(),
        ability_path: vec![AbilityId::new("triggered_01").unwrap()],
    });
    let error = CompiledProgram::compile(kyoshi_owner, EffectContext::Ability, &wrong_type)
        .expect_err("a CreatedTokenBatch cannot satisfy the TokenCopyBatch consumer");
    assert!(
        error.contains("wrong type for this instruction"),
        "unexpected wrong-result-type error: {error}"
    );
}

#[test]
fn mimic_vat_copy_program_allows_arbitrary_and_discarded_batch_bindings() {
    let actual = actual_create_copy_program();
    let mut renamed = actual.clone();
    renamed.body.steps[0].bind[0] = "arbitrary_batch_name".into();
    renamed.body.steps[1].input = Some("arbitrary_batch_name".into());
    renamed.body.steps[2].input = Some("arbitrary_batch_name".into());
    let compiled = compile_mimic_vat_create_copy(&renamed)
        .expect("the typed batch is independent of its authored binding name");
    assert_eq!(compiled.slots.len(), 1);
    assert_eq!(
        compiled.body.steps[1].input,
        compiled.body.steps[0].outputs.first().copied()
    );
    assert_eq!(
        compiled.body.steps[2].input,
        compiled.body.steps[0].outputs.first().copied()
    );

    let mut discarded = actual;
    discarded.body.steps.truncate(1);
    discarded.body.steps[0].bind.clear();
    let compiled = compile_mimic_vat_create_copy(&discarded)
        .expect("an unused producer result remains discardable");
    assert!(compiled.slots.is_empty());
}
