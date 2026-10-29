use tricerules_cards::primitives::{
    Amount, CastTriggerPlayer, CompiledInstructionKind, EffectSubject, PermanentTypeFilter,
    ProgramRequiredInput, ResultType, SpellEffectKind, StaticAbilityDef, TokenCopySource,
    TriggerCondition,
};
use tricerules_cards::{
    AbilityDefinitionId, AbilityId, AbilityLinkId, CardFaceId, Layout, ManaCost, ProgramOwnerId,
};

#[test]
fn molten_echoes_copy_matching_creature_is_a_mandatory_typed_program() {
    let card = tricerules_cards::registry::global()
        .get("molten_echoes")
        .expect("Molten Echoes needs a complete definition");
    assert_eq!(card.name, "Molten Echoes");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "molten_echoes");
    assert_eq!(face.mana_cost, ManaCost::parse("{2}{R}{R}").unwrap());
    assert_eq!(face.types, ["Enchantment"]);
    let [choose_type] = face.static_abilities.as_slice() else {
        panic!("Molten Echoes has exactly one as-enters type choice");
    };
    assert_eq!(choose_type.ability_id.as_str(), "choose_type");
    assert!(matches!(
        &choose_type.definition,
        StaticAbilityDef::AsEntersChooseCreatureType { link_id }
            if link_id == &AbilityLinkId::new("chosen_type").unwrap()
    ));

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Molten Echoes has exactly one triggered ability");
    };
    assert_eq!(trigger.ability_id.as_str(), "copy_matching_creature");
    assert!(matches!(
        &trigger.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter,
            creature_filter: None,
        } if filter.permanent_type == Some(PermanentTypeFilter::Creature)
            && filter.token == Some(false)
            && filter.source_chosen_creature_type.as_ref()
                == Some(&AbilityLinkId::new("chosen_type").unwrap())
    ));
    assert!(trigger.effect.is_empty());

    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "molten_echoes".into(),
        face_id: CardFaceId::new("molten_echoes").unwrap(),
        ability_path: vec![AbilityId::new("copy_matching_creature").unwrap()],
    });
    let program = tricerules_cards::registry::global()
        .compiled_program_for_owner(&owner)
        .expect("the exact triggered ability owns its compiled program");
    assert_eq!(
        program.required_inputs,
        [ProgramRequiredInput::TriggerObservedObject]
    );
    let [copy, haste, delayed] = program.body.steps.as_slice() else {
        panic!("the mandatory program copies, grants haste, then schedules exile");
    };
    assert_eq!(copy.id.as_str(), "copy_entry");
    assert_eq!(haste.id.as_str(), "grant_haste");
    assert_eq!(delayed.id.as_str(), "schedule_exile");
    assert!(matches!(
        &copy.kind,
        CompiledInstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::CreateTokenCopies {
                count: Amount::Fixed(1),
                source: TokenCopySource::TriggerObject,
                copy_exception: None,
            })
    ));
    let [batch_slot] = copy.outputs.as_slice() else {
        panic!("the copy instruction publishes one typed batch slot");
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
                subject: EffectSubject::PreviousEffectObjects,
                modifier: tricerules_cards::ResolvingPermanentModifier::GrantKeywords(keywords),
                duration: tricerules_cards::ResolvingEffectDuration::Indefinite,
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
