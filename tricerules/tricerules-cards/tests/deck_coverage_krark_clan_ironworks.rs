use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, ManaAmount, PermanentTypeFilter, SpellEffectKind,
    TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry};

#[test]
fn krark_clan_ironworks_registers_its_artifact_sacrifice_mana_ability() {
    let registry = CardRegistry::global();
    let card = registry
        .get("krark-clan_ironworks")
        .expect("Krark-Clan Ironworks registry definition");

    assert_eq!(card.name, "Krark-Clan Ironworks");
    assert_eq!(
        registry.id_for_name("Krark-Clan Ironworks"),
        Some("krark-clan_ironworks")
    );
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "krark_clan_ironworks");
    assert_eq!(face.mana_cost.to_string(), "{4}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.colors().is_empty());
    assert_eq!(face.activated_abilities.len(), 1);

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Krark-Clan Ironworks has one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let [AbilityCost::SacrificePermanent { filter }] = ability.costs.as_slice() else {
        panic!("the ability sacrifices one artifact");
    };
    assert_eq!(filter.kind, TargetKind::AnyPermanent);
    assert_eq!(filter.controller, TargetController::You);
    assert_eq!(filter.permanent_types, [PermanentTypeFilter::Artifact]);
    assert!(filter.excluded_objects.is_empty());
    assert!(ability.targeting.is_none());
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::ProduceMana { options, .. }]
            if options.as_slice() == [ManaAmount { c: 2, ..ManaAmount::default() }]
    ));
    assert_eq!(
        ability.mana_options().map(Vec::as_slice),
        Some(
            &[ManaAmount {
                c: 2,
                ..ManaAmount::default()
            }][..]
        )
    );
}
