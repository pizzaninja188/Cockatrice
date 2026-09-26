use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, CombatRestriction, CombatRestrictionScope, EffectSubject,
    PermanentTypeFilter, SpellEffectKind, TargetFilter, TargetKind, TargetObjectExclusion,
};
use tricerules_cards::CardRegistry;

#[test]
fn manifold_key_registers_its_artifact_untap_and_unblockable_activations() {
    let registry = CardRegistry::global();
    let card = registry.get("manifold_key").expect("Manifold Key");
    let face = card.primary_face();
    assert_eq!(face.name, "Manifold Key");
    assert_eq!(face.mana_cost.to_string(), "{1}");
    assert_eq!(face.types, ["Artifact"]);

    let [untap, unblockable] = face.activated_abilities.as_slice() else {
        panic!("Manifold Key has two activated abilities");
    };
    assert_eq!(untap.source_zone, AbilitySourceZone::Battlefield);
    assert!(matches!(
        untap.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{1}"
    ));
    let untap_filter = TargetFilter {
        kind: TargetKind::AnyPermanent,
        permanent_types: vec![PermanentTypeFilter::Artifact],
        excluded_objects: vec![TargetObjectExclusion::Source],
        ..TargetFilter::default()
    };
    assert_eq!(
        untap.effect,
        [SpellEffectKind::Untap {
            subject: EffectSubject::Chosen(Box::new(untap_filter.clone())),
        }]
    );
    let untap_targeting = untap.targeting.as_ref().expect("artifact target group");
    let [untap_group] = untap_targeting.groups.as_slice() else {
        panic!("Manifold Key's first ability has one target group");
    };
    assert_eq!((untap_group.min, untap_group.max), (1, 1));
    assert_eq!(untap_group.prompt, "Choose another target artifact");
    assert_eq!(untap_group.effect_indices, [0]);

    assert_eq!(unblockable.source_zone, AbilitySourceZone::Battlefield);
    assert!(matches!(
        unblockable.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{3}"
    ));
    assert!(matches!(
        unblockable.effect.as_slice(),
        [SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Chosen(filter),
            restriction: CombatRestriction { cant_be_blocked: true, .. },
        }] if filter.kind == TargetKind::Creature
    ));
    let unblockable_targeting = unblockable
        .targeting
        .as_ref()
        .expect("creature target group");
    let [unblockable_group] = unblockable_targeting.groups.as_slice() else {
        panic!("Manifold Key's second ability has one target group");
    };
    assert_eq!((unblockable_group.min, unblockable_group.max), (1, 1));
    assert_eq!(unblockable_group.prompt, "Choose target creature");
    assert_eq!(unblockable_group.effect_indices, [0]);
}
