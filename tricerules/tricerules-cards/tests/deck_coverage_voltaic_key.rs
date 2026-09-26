use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, PermanentTypeFilter, SpellEffectKind, TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

#[test]
fn voltaic_key_registers_its_targeted_artifact_untap_ability() {
    let registry = CardRegistry::global();
    let card = registry
        .get("voltaic_key")
        .expect("Voltaic Key registry definition");

    assert_eq!(card.name, "Voltaic Key");
    assert_eq!(registry.id_for_name("Voltaic Key"), Some("voltaic_key"));
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "voltaic_key");
    assert_eq!(face.mana_cost.to_string(), "{1}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.colors().is_empty());
    assert!(face.spell_effect.is_empty());
    assert!(face.triggered_abilities.is_empty());
    assert_eq!(face.activated_abilities.len(), 1);

    let ability = &face.activated_abilities[0];
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{1}"
    ));
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::Untap {
            subject: EffectSubject::Chosen(filter)
        }] if filter.kind == TargetKind::AnyPermanent
            && filter.permanent_types.as_slice() == [PermanentTypeFilter::Artifact]
            && filter.excluded_objects.is_empty()
    ));
}
