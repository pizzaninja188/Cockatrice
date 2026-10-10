use tricerules_cards::primitives::{
    CardTypeFilter, EventZone, GraveyardDestination, GraveyardOwner, PermanentTypeFilter,
    SpellEffectKind, TriggerCondition, ZoneEventCardinality, ZoneEventDestination,
};

#[test]
fn scrap_trawler_registers_as_a_complete_card() {
    let definition = tricerules_cards::registry::global()
        .get("scrap_trawler")
        .expect("Scrap Trawler");
    let face = definition.primary_face();
    assert_eq!(face.name, "Scrap Trawler");
    assert_eq!(face.face_id.as_str(), "scrap_trawler");
    assert_eq!(face.mana_cost.to_string(), "{3}");
    assert_eq!(
        face.types.iter().map(String::as_str).collect::<Vec<_>>(),
        ["Artifact", "Creature", "Construct"]
    );
    assert_eq!((face.power, face.toughness), (Some(3), Some(2)));

    let [self_death, other_artifact] = face.triggered_abilities.as_slice() else {
        panic!("Scrap Trawler has a self-death and an other-artifact trigger");
    };
    assert_eq!(self_death.trigger, TriggerCondition::WhenSelfDies);
    assert!(matches!(
        &other_artifact.trigger,
        TriggerCondition::WheneverPermanentLeavesBattlefield {
            filter,
            destination: ZoneEventDestination::OneOf(destinations),
            cardinality: ZoneEventCardinality::EachObject,
            ..
        } if filter.permanent_type == Some(PermanentTypeFilter::Artifact)
            && filter.exclude_source
            && destinations == &[EventZone::Graveyard]
    ));

    for ability in [self_death, other_artifact] {
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::MoveGraveyardCards {
                filter,
                destination: GraveyardDestination::Hand,
                ..
            }] if filter.owner == GraveyardOwner::Controller
                && filter.card.as_ref().is_some_and(|card| card.card_type == Some(CardTypeFilter::Artifact))
                && filter.mana_value_less_than_trigger_object
        ));
        let targeting = ability
            .targeting
            .as_ref()
            .expect("the trigger has a target");
        let [group] = targeting.groups.as_slice() else {
            panic!("each Scrap Trawler trigger has one target group");
        };
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(group.effect_indices, [0]);
    }
}
