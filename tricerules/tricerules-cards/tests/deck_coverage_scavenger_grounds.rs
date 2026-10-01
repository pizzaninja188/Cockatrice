use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, RelativePlayerSet, SpellEffectKind, TargetController,
    TargetKind,
};
use tricerules_cards::CardRegistry;

#[test]
fn scavenger_grounds_registers_colorless_mana_and_all_graveyards_exile() {
    let card = CardRegistry::global()
        .get("scavenger_grounds")
        .expect("Scavenger Grounds is registered");
    let face = card.primary_face();
    assert_eq!(face.name, "Scavenger Grounds");
    assert!(face.mana_cost.to_string().is_empty());
    assert_eq!(face.types, ["Land", "Desert"]);

    let [colorless, graveyard_exile] = face.activated_abilities.as_slice() else {
        panic!("Scavenger Grounds has a mana ability and a graveyard-exile ability");
    };
    assert_eq!(colorless.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(colorless.costs, [AbilityCost::Tap]);
    assert!(colorless.targeting.is_none());
    assert!(matches!(
        colorless.effect.as_slice(),
        [SpellEffectKind::ProduceMana {
            commander_color_identity: false,
            options,
            restriction: None,
            conditional: None,
        }] if options.len() == 1
            && options[0].w == 0
            && options[0].u == 0
            && options[0].b == 0
            && options[0].r == 0
            && options[0].g == 0
            && options[0].c == 1
    ));

    assert_eq!(graveyard_exile.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(graveyard_exile.costs.len(), 3);
    let [mana, tap, sacrifice] = graveyard_exile.costs.as_slice() else {
        panic!("graveyard exile costs {{2}}, tap, and a Desert sacrifice");
    };
    assert!(matches!(mana, AbilityCost::Mana(cost) if cost.to_string() == "{2}"));
    assert_eq!(*tap, AbilityCost::Tap);
    let AbilityCost::SacrificePermanent { filter, count: 1 } = sacrifice else {
        panic!("the final cost sacrifices a controlled Desert");
    };
    assert_eq!(filter.kind, TargetKind::AnyPermanent);
    assert_eq!(filter.controller, TargetController::You);
    assert_eq!(filter.required_subtypes, ["Desert"]);
    assert!(
        filter.excluded_objects.is_empty(),
        "the source can pay the cost"
    );

    assert!(
        graveyard_exile.targeting.is_none(),
        "the ability has no targets"
    );
    assert_eq!(
        graveyard_exile.effect,
        [SpellEffectKind::ExileGraveyards {
            players: RelativePlayerSet::All,
            filter: None,
        }]
    );
}
