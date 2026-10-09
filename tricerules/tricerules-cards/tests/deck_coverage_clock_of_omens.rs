use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, EffectSubject, ObjectPaymentConstraint, PermanentTypeFilter,
    SpellEffectKind, TargetController, TargetFilter, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Layout};

#[test]
fn clock_of_omens_registers_its_complete_artifact_untap_ability() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("clock_of_omens")
        .expect("Clock of Omens registry definition");

    assert_eq!(card.name, "Clock of Omens");
    assert_eq!(
        registry.id_for_name("Clock of Omens"),
        Some("clock_of_omens")
    );
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "clock_of_omens");
    assert_eq!(face.mana_cost.to_string(), "{4}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.colors().is_empty());
    assert!(face.spell_effect.is_empty());
    assert!(face.triggered_abilities.is_empty());
    assert_eq!(face.activated_abilities.len(), 1);

    let ability = &face.activated_abilities[0];
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::TapPermanents {
            constraint: ObjectPaymentConstraint::ExactCount(2),
            filter,
            exclude_source: false,
        }] if filter.kind == TargetKind::AnyPermanent
            && filter.controller == TargetController::You
            && filter.permanent_types.as_slice() == [PermanentTypeFilter::Artifact]
    ));
    let target_filter = TargetFilter {
        kind: TargetKind::AnyPermanent,
        permanent_types: vec![PermanentTypeFilter::Artifact],
        ..TargetFilter::default()
    };
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Untap {
            subject: EffectSubject::Chosen(Box::new(target_filter)),
        }]
    );
}
