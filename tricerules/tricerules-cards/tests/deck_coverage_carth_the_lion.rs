//! Carth's exact identity and mandatory entry trigger with optional selection.
use tricerules_cards::primitives::{
    CardTypeFilter, CastTriggerPlayer, EventZone, LibraryBottomOrder, PermanentEventFilter,
    PermanentTypeFilter, SpellEffectKind, StaticAbilityDef, TriggerCondition, ZoneCardFilter,
    ZoneEventCardinality, ZoneEventDestination,
};
use tricerules_cards::AbilityPresentation;

#[test]
fn carth_exact_identity_and_entry_look() {
    let registry = tricerules_cards::registry::global();
    let id = registry
        .id_for_name("Carth the Lion")
        .expect("Carth the Lion must be registered");
    assert_eq!(id, "carth_the_lion");
    let card = registry.get(id).unwrap();
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.name, "Carth the Lion");
    assert_eq!(face.face_id.as_str(), "carth_the_lion");
    assert_eq!(face.mana_cost.to_string(), "{2}{B}{G}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Creature", "Human", "Warrior"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(5)));

    let ability = face
        .triggered_abilities
        .iter()
        .find(|ability| ability.trigger == TriggerCondition::WhenSelfEntersBattlefield)
        .expect("Carth has a self-entry trigger");
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(
        !ability.may,
        "looking is mandatory; the card selection is optional"
    );
    assert!(ability.targeting.is_none() && ability.program.is_none());
    assert_eq!(
        ability.effect,
        [SpellEffectKind::LookChooseToHand {
            count: 7,
            filter: Some(ZoneCardFilter {
                card_type: Some(CardTypeFilter::Planeswalker),
                ..Default::default()
            }),
            min: 0,
            max: 1,
            reveal: true,
            bottom_order: LibraryBottomOrder::Random,
        }]
    );
}

#[test]
fn carth_has_the_scoped_additional_loyalty_cost_static() {
    let card = tricerules_cards::registry::global()
        .get("carth_the_lion")
        .expect("Carth the Lion must be registered");
    let [ability] = card.primary_face().static_abilities.as_slice() else {
        panic!("one Carth loyalty cost static")
    };
    assert_eq!(ability.ability_id.as_str(), "static_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.definition,
        StaticAbilityDef::ControllerPlaneswalkerLoyaltyCostsAdditionalOne
    );
}

#[test]
fn carth_planeswalker_death_reuses_the_entry_look_for_each_controlled_walker() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("carth_the_lion")
        .expect("Carth the Lion must be registered");
    let face = card.primary_face();
    let entry = face
        .triggered_abilities
        .iter()
        .find(|ability| ability.trigger == TriggerCondition::WhenSelfEntersBattlefield)
        .expect("self-entry trigger");
    let death = face
        .triggered_abilities
        .iter()
        .find(|ability| {
            matches!(
                &ability.trigger,
                TriggerCondition::WheneverPermanentLeavesBattlefield { .. }
            )
        })
        .expect("Carth has a controlled-planeswalker death trigger");
    assert_eq!(death.ability_id.as_str(), "triggered_02");
    assert_eq!(
        death.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        death.trigger,
        TriggerCondition::WheneverPermanentLeavesBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                permanent_type: Some(PermanentTypeFilter::Planeswalker),
                ..Default::default()
            },
            destination: ZoneEventDestination::OneOf(vec![EventZone::Graveyard]),
            cardinality: ZoneEventCardinality::EachObject,
        }
    );
    assert!(!death.may && death.targeting.is_none() && death.program.is_none());
    assert_eq!(death.effect, entry.effect);
}
