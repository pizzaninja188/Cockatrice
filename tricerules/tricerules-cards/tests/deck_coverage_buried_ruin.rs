use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, GraveyardDestination, GraveyardOwner, SpellEffectKind,
};
use tricerules_cards::CardRegistry;

#[test]
fn buried_ruin_registers_colorless_mana_and_targeted_artifact_recovery() {
    let registry = CardRegistry::global();
    let card = registry.get("buried_ruin").expect("Buried Ruin");
    let face = card.primary_face();
    assert_eq!(face.name, "Buried Ruin");
    assert_eq!(face.mana_cost.to_string(), "");
    assert_eq!(face.types, ["Land"]);

    let [colorless, recovery] = face.activated_abilities.as_slice() else {
        panic!("Buried Ruin has a mana ability and an artifact recovery ability");
    };
    assert_eq!(colorless.source_zone, AbilitySourceZone::Battlefield);
    assert!(matches!(colorless.costs.as_slice(), [AbilityCost::Tap]));
    assert!(matches!(
        colorless.effect.as_slice(),
        [SpellEffectKind::ProduceMana { options, .. }]
            if options.len() == 1 && options[0].c == 1
    ));

    assert_eq!(recovery.source_zone, AbilitySourceZone::Battlefield);
    assert!(matches!(
        recovery.costs.as_slice(),
        [AbilityCost::Mana(mana), AbilityCost::Tap, AbilityCost::SacrificeSelf]
            if mana.to_string() == "{2}"
    ));
    assert!(matches!(
        recovery.effect.as_slice(),
        [SpellEffectKind::MoveGraveyardCards {
            filter,
            destination: GraveyardDestination::Hand,
            ..
        }] if filter.owner == GraveyardOwner::Controller && filter.card.is_some()
    ));
    let targeting = recovery.targeting.as_ref().expect("one target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Buried Ruin has one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(
        group.prompt,
        "Choose target artifact card from your graveyard"
    );
    assert_eq!(group.effect_indices, [0]);
}
