use tricerules_cards::primitives::{
    AbilityCost, CastTriggerPlayer, EffectSubject, EventZone, PermanentTypeFilter,
    ReturnController, SpellEffectKind, TriggerCondition, TriggeredCardReference,
    ZoneEventCardinality, ZoneEventDestination,
};
use tricerules_cards::{AbilityDefinitionId, AbilityId, CardFaceId, ProgramOwnerId};

#[test]
fn daretti_complete_definition_maps_the_three_loyalty_abilities() {
    let registry = tricerules_cards::registry::global();
    let card = registry.get("daretti,_scrap_savant").unwrap();
    let face = card.primary_face();
    assert!(!card.commander_setup_only);
    assert_eq!(face.name, "Daretti, Scrap Savant");
    assert_eq!(face.face_id.as_str(), "daretti_scrap_savant");
    assert_eq!(face.mana_cost.to_string(), "{3}{R}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Daretti"]);
    assert_eq!(face.loyalty, Some(3));
    let [plus, minus, ultimate] = face.activated_abilities.as_slice() else {
        panic!("Daretti has three loyalty abilities");
    };
    assert_eq!(plus.costs, [AbilityCost::Loyalty(2)]);
    assert_eq!(minus.costs, [AbilityCost::Loyalty(-2)]);
    assert_eq!(ultimate.costs, [AbilityCost::Loyalty(-10)]);
    for id in ["activated_01", "activated_02"] {
        let owner = ProgramOwnerId::ability(AbilityDefinitionId {
            card_id: card.id.clone(),
            face_id: CardFaceId::new("daretti_scrap_savant").unwrap(),
            ability_path: vec![AbilityId::new(id).unwrap()],
        });
        assert!(registry.compiled_program_for_owner(&owner).is_some());
    }
    let [SpellEffectKind::CreateTriggeredEmblem { ability, .. }] = ultimate.effect.as_slice()
    else {
        panic!("ultimate creates the emblem");
    };
    assert!(matches!(&ability.trigger,
        TriggerCondition::WheneverPermanentLeavesBattlefield {
            controller: CastTriggerPlayer::AnyPlayer, filter,
            destination: ZoneEventDestination::OneOf(zones), cardinality: ZoneEventCardinality::EachObject,
        } if filter.permanent_type == Some(PermanentTypeFilter::Artifact)
            && filter.owner == Some(CastTriggerPlayer::Controller)
            && zones == &[EventZone::Graveyard]));
    let [SpellEffectKind::CreateDelayedTrigger {
        subject: Some(EffectSubject::TriggerObject),
        ability: delayed,
        ..
    }] = ability.effect.as_slice()
    else {
        panic!("emblem creates the delayed return from its observed artifact");
    };
    assert_eq!(delayed.trigger, TriggerCondition::AtBeginningOfNextEndStep);
    assert!(
        matches!(delayed.effect.as_slice(), [SpellEffectKind::ReturnTriggeredCard {
        reference: TriggeredCardReference::ExactTriggerObject, from,
        controller: ReturnController::AbilityController, ..
    }] if from == &[EventZone::Graveyard])
    );
}
