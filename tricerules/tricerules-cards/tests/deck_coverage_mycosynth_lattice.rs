//! Exact identity and all three static abilities of Mycosynth Lattice.
use tricerules_cards::primitives::{StaticAbilityDef, TargetKind};
use tricerules_cards::{AbilityPresentation, PermanentTypeFilter};

#[test]
fn lattice_exact_identity_and_all_three_static_abilities() {
    let registry = tricerules_cards::registry::global();
    let id = registry
        .id_for_name("Mycosynth Lattice")
        .expect("exact Mycosynth Lattice identity must be registered");
    assert_eq!(id, "mycosynth_lattice");
    let card = registry.get(id).unwrap();
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.name, "Mycosynth Lattice");
    assert_eq!(face.face_id.as_str(), "mycosynth_lattice");
    assert_eq!(face.mana_cost.to_string(), "{6}");
    assert_eq!(face.types, ["Artifact"]);
    assert_eq!(face.static_abilities.len(), 3);
    let ability = face.static_abilities.first().expect("first static ability");
    assert_eq!(ability.ability_id.as_str(), "static_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let StaticAbilityDef::AddTypesToPermanents { filter, addition } = &ability.definition else {
        panic!("all-permanent artifact addition");
    };
    assert_eq!(filter.kind, TargetKind::AnyPermanent);
    assert!(filter.permanent_types.is_empty());
    assert_eq!(addition.card_types, [PermanentTypeFilter::Artifact]);
    assert!(addition.creature_types.is_empty() && addition.land_types.is_empty());
    let color = &face.static_abilities[1];
    assert_eq!(color.ability_id.as_str(), "static_02");
    assert_eq!(
        color.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        &color.definition,
        StaticAbilityDef::AllCardsSpellsAndPermanentsAreColorless
    ));
    let spending = &face.static_abilities[2];
    assert_eq!(spending.ability_id.as_str(), "static_03");
    assert_eq!(
        spending.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert!(matches!(
        &spending.definition,
        StaticAbilityDef::AllPlayersMaySpendManaAsThoughAnyColor
    ));
}
