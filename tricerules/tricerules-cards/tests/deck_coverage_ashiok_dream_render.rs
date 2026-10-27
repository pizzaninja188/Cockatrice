//! Exact Ashiok identity and the ordered -1 loyalty ability.
use tricerules_cards::primitives::{
    AbilityCost, RelativePlayerSet, SpellEffectKind, StaticAbilityDef, TargetKind,
};
use tricerules_cards::AbilityPresentation;

#[test]
fn ashiok_exact_identity_hybrid_cost_and_minus_one() {
    let registry = tricerules_cards::registry::global();
    let id = registry
        .id_for_name("Ashiok, Dream Render")
        .expect("Ashiok, Dream Render must be registered");
    assert_eq!(id, "ashiok,_dream_render");
    let card = registry.get(id).unwrap();
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.name, "Ashiok, Dream Render");
    assert_eq!(face.face_id.as_str(), "ashiok_dream_render");
    assert_eq!(face.mana_cost.to_string(), "{1}{U/B}{U/B}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Ashiok"]);
    assert_eq!(face.loyalty, Some(5));
    // The model also materializes printed loyalty as an intrinsic entry replacement.
    let restrictions: Vec<_> = face
        .static_abilities
        .iter()
        .filter(|ability| {
            matches!(
                ability.definition,
                StaticAbilityDef::OpponentsCannotCauseOwnLibrarySearch
            )
        })
        .collect();
    let [restriction] = restrictions.as_slice() else {
        panic!("one search restriction")
    };
    assert_eq!(restriction.ability_id.as_str(), "static_01");
    assert_eq!(
        restriction.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        restriction.definition,
        StaticAbilityDef::OpponentsCannotCauseOwnLibrarySearch
    );
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("one loyalty ability")
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.costs, [AbilityCost::Loyalty(-1)]);
    assert!(matches!(ability.effect.as_slice(), [
        SpellEffectKind::MillTargetPlayer { count: 4, target },
        SpellEffectKind::ExileGraveyards { players: RelativePlayerSet::Opponents, filter: None, linked_exile_id: None, capture_exile_cohort: None },
    ] if target.kind == TargetKind::AnyPlayer));
}
