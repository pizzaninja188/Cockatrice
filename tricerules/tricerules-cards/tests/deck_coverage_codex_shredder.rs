use tricerules_cards::primitives::{
    AbilityCost, GraveyardDestination, GraveyardOwner, SpellEffectKind, TargetKind,
};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone};

#[test]
fn codex_shredder_registers_its_mill_and_recovery_abilities() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("codex_shredder")
        .expect("Codex Shredder registry definition");

    assert_eq!(card.name, "Codex Shredder");
    assert_eq!(
        registry.id_for_name("Codex Shredder"),
        Some("codex_shredder")
    );
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "codex_shredder");
    assert_eq!(face.mana_cost.to_string(), "{1}");
    assert_eq!(face.types, ["Artifact"]);
    let [mill, recovery] = face.activated_abilities.as_slice() else {
        panic!("Codex Shredder has two activated abilities");
    };

    assert_eq!(mill.ability_id.as_str(), "activated_01");
    assert_eq!(mill.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(mill.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(mill.costs, [AbilityCost::Tap]);
    assert!(matches!(
        mill.effect.as_slice(),
        [SpellEffectKind::MillTargetPlayer { count: 1, target }]
            if target.kind == TargetKind::AnyPlayer
    ));
    assert!(mill.targeting.is_none());

    assert_eq!(recovery.ability_id.as_str(), "activated_02");
    assert_eq!(recovery.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        recovery.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        recovery.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap, AbilityCost::SacrificeSelf]
            if cost.to_string() == "{5}"
    ));
    assert!(matches!(
        recovery.effect.as_slice(),
        [SpellEffectKind::MoveGraveyardCards {
            filter,
            destination: GraveyardDestination::Hand,
            linked_exile_id: None,
        }] if filter.owner == GraveyardOwner::Controller && filter.card.is_none()
    ));
    assert!(recovery.targeting.is_some());
}
