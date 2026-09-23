use tricerules_cards::primitives::{
    GameCondition, GraveyardDestination, GraveyardOwner, SpellEffectKind, TargetSchema,
    TriggerCondition,
};
use tricerules_cards::CardRegistry;

#[test]
fn anti_venom_has_the_complete_cast_conditioned_graveyard_return() {
    let face = CardRegistry::global()
        .get("anti-venom,_horrifying_healer")
        .expect("Anti-Venom, Horrifying Healer is registered")
        .primary_face();
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Anti-Venom has one cast-conditioned ETB ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(matches!(
        ability.intervening_if.as_ref(),
        Some(GameCondition::SelfWasCast)
    ));
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::MoveGraveyardCards {
            filter,
            destination: GraveyardDestination::Battlefield { tapped: false, .. },
            ..
        }] if filter.owner == GraveyardOwner::Controller
            && filter.card.as_ref().is_some_and(|card| card.card_type == Some(tricerules_cards::primitives::CardTypeFilter::Creature))
    ));
    let targeting = ability
        .targeting
        .as_ref()
        .expect("required creature target");
    let [group] = targeting.groups.as_slice() else {
        panic!("Anti-Venom has one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(
        group.prompt,
        "Choose target creature card in your graveyard"
    );
    assert!(TargetSchema::compile(&ability.effect, Some(targeting)).is_ok());
}
