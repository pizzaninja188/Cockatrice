//! Exact Onakke identity, attack tax, and graveyard return definition.
use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, ActivationTiming, AttackTaxDefender, CardTypeFilter,
    GraveyardDestination, GraveyardOwner, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::AbilityPresentation;

#[test]
fn onakke_exact_identity_and_both_abilities() {
    let registry = tricerules_cards::registry::global();
    let id = registry
        .id_for_name("Onakke Oathkeeper")
        .expect("Onakke Oathkeeper must be registered");
    assert_eq!(id, "onakke_oathkeeper");
    let card = registry.get(id).unwrap();
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.name, "Onakke Oathkeeper");
    assert_eq!(face.face_id.as_str(), "onakke_oathkeeper");
    assert_eq!(face.mana_cost.to_string(), "{1}{W}");
    assert_eq!(face.types, ["Creature", "Ogre", "Spirit"]);
    assert_eq!((face.power, face.toughness), (Some(0), Some(4)));
    assert_eq!(face.static_abilities.len(), 1);
    let tax = &face.static_abilities[0];
    assert_eq!(tax.ability_id.as_str(), "static_01");
    assert_eq!(tax.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert!(matches!(
        &tax.definition,
        StaticAbilityDef::AttackTax {
            generic_per_attacker: 1,
            defender: AttackTaxDefender::ControllerPlaneswalkers,
        }
    ));

    assert_eq!(face.activated_abilities.len(), 1);
    let ability = &face.activated_abilities[0];
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Graveyard);
    assert_eq!(ability.timing, ActivationTiming::Normal);
    assert!(matches!(ability.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::ExileSelf] if cost.to_string() == "{4}{W}{W}"));
    assert!(ability.program.is_none());
    let [SpellEffectKind::MoveGraveyardCards {
        filter,
        destination,
        ..
    }] = ability.effect.as_slice()
    else {
        panic!("Onakke returns a targeted planeswalker card from its controller's graveyard");
    };
    assert_eq!(filter.owner, GraveyardOwner::Controller);
    assert_eq!(
        filter.card.as_ref().and_then(|card| card.card_type),
        Some(CardTypeFilter::Planeswalker)
    );
    assert_eq!(
        *destination,
        GraveyardDestination::Battlefield { tapped: false }
    );
}
