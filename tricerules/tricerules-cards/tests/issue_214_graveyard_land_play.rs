use tricerules_cards::primitives::{Amount, PlayerRecipient, SpellEffectKind, StaticAbilityDef};
use tricerules_cards::{
    AbilityPresentation, CardRegistry, CastTriggerPlayer, PermanentTypeFilter, TriggerCondition,
};

#[test]
fn icetill_explorer_has_the_exact_reusable_ability_shape() {
    let card = CardRegistry::global()
        .get("icetill_explorer")
        .expect("Icetill Explorer");
    let face = card.primary_face();

    assert_eq!(card.name, "Icetill Explorer");
    assert_eq!(face.face_id.as_str(), "icetill_explorer");
    assert_eq!(face.mana_cost.to_string(), "{2}{G}{G}");
    assert_eq!(face.types, ["Creature", "Insect", "Scout"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(4)));
    assert_eq!(face.static_abilities.len(), 2);
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        face.static_abilities[0].definition,
        StaticAbilityDef::ExtraLandPlays { count: 1 }
    ));
    assert_eq!(
        face.static_abilities[1].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        face.static_abilities[1].definition,
        StaticAbilityDef::PlayLandsFromOwnGraveyard
    );

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("one landfall trigger");
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    let TriggerCondition::WheneverPermanentEntersBattlefield {
        controller,
        filter,
        creature_filter,
    } = &trigger.trigger
    else {
        panic!("landfall trigger");
    };
    assert_eq!(*controller, CastTriggerPlayer::Controller);
    assert_eq!(filter.permanent_type, Some(PermanentTypeFilter::Land));
    assert!(creature_filter.is_none());
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::Mill {
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
        }]
    ));
}
