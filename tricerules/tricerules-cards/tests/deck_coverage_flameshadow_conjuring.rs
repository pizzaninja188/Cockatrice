use tricerules_cards::primitives::{
    Amount, CastTriggerPlayer, CompiledChooseRequirement, CompiledInstructionKind, EffectSubject,
    PermanentTypeFilter, PlayerRecipient, ProgramRequiredInput, ResolutionCost, ResultType,
    SpellEffectKind, TokenCopySource, TriggerCondition,
};
use tricerules_cards::{
    AbilityDefinitionId, AbilityId, CardFaceId, Layout, ManaCost, ProgramOwnerId,
};

#[test]
fn flameshadow_conjuring_is_registered_as_a_complete_card() {
    let card = tricerules_cards::registry::global()
        .get("flameshadow_conjuring")
        .expect("Flameshadow Conjuring needs a complete definition");
    assert_eq!(card.name, "Flameshadow Conjuring");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "flameshadow_conjuring");
    assert_eq!(face.mana_cost, ManaCost::parse("{3}{R}").unwrap());
    assert_eq!(face.types, ["Enchantment"]);
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Flameshadow Conjuring has exactly one triggered ability");
    };
    assert!(matches!(
        &trigger.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield { controller: CastTriggerPlayer::Controller, filter, creature_filter: None }
            if filter.permanent_type == Some(PermanentTypeFilter::Creature)
                && filter.token == Some(false)
                && !filter.exclude_source
    ));
    assert!(trigger.effect.is_empty());
    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "flameshadow_conjuring".into(),
        face_id: CardFaceId::new("flameshadow_conjuring").unwrap(),
        ability_path: vec![AbilityId::new("triggered_01").unwrap()],
    });
    let program = tricerules_cards::registry::global()
        .compiled_program_for_owner(&owner)
        .expect("the exact triggered ability owns its compiled program");
    assert_eq!(
        program.required_inputs,
        [ProgramRequiredInput::TriggerObservedObject]
    );
    let [choose] = program.body.steps.as_slice() else {
        panic!("the program contains one optional copy choice");
    };
    let CompiledInstructionKind::Choose {
        choice,
        chooser,
        optional,
        branches,
        otherwise,
        ..
    } = &choose.kind
    else {
        panic!("the triggered program starts with Choose");
    };
    assert_eq!(choice.as_str(), "choose_copy");
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(*optional);
    assert!(otherwise.steps.is_empty());
    let [branch] = branches.as_slice() else {
        panic!("the choice has one paid branch");
    };
    assert_eq!(branch.branch_id.as_str(), "pay_to_copy");
    assert_eq!(
        branch.cost,
        ResolutionCost::Mana(ManaCost::parse("{R}").unwrap())
    );
    assert_eq!(branch.requirement, CompiledChooseRequirement::Always);
    let [copy, haste, delayed] = branch.body.steps.as_slice() else {
        panic!("the paid branch copies, grants haste, then schedules exile");
    };
    assert!(matches!(
        &copy.kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::CreateTokenCopies {
                count: Amount::Fixed(1), source: TokenCopySource::TriggerObject,
                copy_exception: None,
            })
    ));
    let [batch_slot] = copy.outputs.as_slice() else {
        panic!("the exact copy instruction publishes one typed batch slot");
    };
    assert!(matches!(
        program.slot(*batch_slot).map(|slot| &slot.result_type),
        Some(ResultType::TokenCopyBatch)
    ));
    assert_eq!(haste.input, Some(*batch_slot));
    assert!(matches!(
        &haste.kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::ApplyPermanentModifier {
                modifier: tricerules_cards::ResolvingPermanentModifier::GrantKeywords(keywords),
                duration: tricerules_cards::ResolvingEffectDuration::Indefinite,
                ..
            } if keywords == &[tricerules_cards::Keyword::Haste])
    ));
    assert_eq!(delayed.input, Some(*batch_slot));
    assert!(matches!(
        &delayed.kind,
        CompiledInstructionKind::Perform(effect)
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
}
