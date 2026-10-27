//! Exact Oath identity and its mandatory entry-trigger composition.
use tricerules_cards::primitives::{
    EffectSubject, EventZone, ReturnController, SpellEffectKind, StaticAbilityDef,
    TargetController, TargetKind, TargetObjectExclusion, TriggerCondition,
    TriggeredCardDestination, TriggeredCardReference,
};
use tricerules_cards::{AbilityPresentation, InstructionKind, ProgramTargetRole};

#[test]
fn oath_exact_identity_and_mandatory_entry_program() {
    let registry = tricerules_cards::registry::global();
    let id = registry
        .id_for_name("Oath of Teferi")
        .expect("Oath of Teferi must be registered");
    assert_eq!(id, "oath_of_teferi");
    let card = registry.get(id).unwrap();
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.name, "Oath of Teferi");
    assert_eq!(face.face_id.as_str(), "oath_of_teferi");
    assert_eq!(face.mana_cost.to_string(), "{3}{W}{U}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Enchantment"]);

    let [loyalty] = face.static_abilities.as_slice() else {
        panic!("Oath has one loyalty permission");
    };
    assert_eq!(loyalty.ability_id.as_str(), "static_01");
    assert_eq!(
        loyalty.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        loyalty.definition,
        StaticAbilityDef::ControllerMayActivatePlaneswalkerLoyaltyTwicePerTurn
    );

    let [entry] = face.triggered_abilities.as_slice() else {
        panic!("Oath has one entry trigger");
    };
    assert_eq!(entry.ability_id.as_str(), "triggered_01");
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(entry.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!entry.may && entry.intervening_if.is_none());
    let [group] = entry
        .targeting
        .as_ref()
        .expect("entry target")
        .groups
        .as_slice()
    else {
        panic!("entry has exactly one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    let program = entry
        .program
        .as_ref()
        .expect("entry uses the exile receipt");
    let [exile, schedule] = program.body.steps.as_slice() else {
        panic!("entry exiles its target and schedules the return");
    };
    assert!(matches!(&exile.kind, InstructionKind::Perform(effect)
        if matches!(effect.as_ref(), SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(filter),
            ..
        } if filter.kind == TargetKind::AnyPermanent
            && filter.controller == TargetController::You
            && filter.excluded_objects == [TargetObjectExclusion::Source])));
    assert_eq!(exile.bind.len(), 1);
    assert_eq!(schedule.input.as_ref(), exile.bind.first());
    let [binding] = program.target_bindings.as_slice() else {
        panic!("the exile subject is bound to the required target");
    };
    assert_eq!(binding.instruction_id, exile.id);
    assert_eq!(binding.role, ProgramTargetRole::Subject);
    assert_eq!(binding.target_group_index, 0);
    let InstructionKind::Perform(effect) = &schedule.kind else {
        panic!("entry schedules a delayed trigger");
    };
    let SpellEffectKind::CreateDelayedTrigger {
        subject: Some(EffectSubject::PreviousEffectObject),
        ability,
        ..
    } = effect.as_ref()
    else {
        panic!("the delayed trigger consumes the exact exile receipt");
    };
    assert_eq!(ability.trigger, TriggerCondition::AtBeginningOfNextEndStep);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(
        matches!(ability.effect.as_slice(), [SpellEffectKind::ReturnTriggeredCard {
        reference: TriggeredCardReference::ExactTriggerObject,
        from,
        destination: TriggeredCardDestination::Battlefield,
        controller: ReturnController::Owner,
        ..
    }] if from.as_slice() == [EventZone::Exile])
    );
}
