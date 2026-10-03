use tricerules_cards::primitives::{
    AbilityCost, CombatRole, EffectSubject, SpellEffectKind, TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry};

#[test]
fn maze_is_the_exact_single_face_land_with_one_shared_attacking_target_and_no_mana_ability() {
    let registry = CardRegistry::global();
    assert_eq!(registry.id_for_name("Maze of Ith"), Some("maze_of_ith"));
    let card = registry.get("maze_of_ith").unwrap();
    assert_eq!(card.name, "Maze of Ith");
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "maze_of_ith");
    assert_eq!(face.types, ["Land"]);
    assert_eq!(face.mana_cost.to_string(), "");
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("one complete ability")
    };
    assert_eq!(ability.costs, [AbilityCost::Tap]);
    assert!(!ability.is_mana_ability());
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let [SpellEffectKind::Untap {
        subject: EffectSubject::Chosen(untap),
    }, SpellEffectKind::PreventAllCombatDamageToTargetTurn { target: incoming }, SpellEffectKind::PreventAllCombatDamageByTargetTurn { target: outgoing }] =
        ability.effect.as_slice()
    else {
        panic!("all three exact instructions")
    };
    assert_eq!(untap.as_ref(), incoming);
    assert_eq!(untap.as_ref(), outgoing);
    assert_eq!(untap.kind, TargetKind::Creature);
    assert_eq!(untap.combat_role, Some(CombatRole::Attacking));
    assert!(ability.targeting.is_none());
    assert!(ability.conditions.is_empty());
    assert!(ability.activation_limit.is_none());
}

#[test]
fn outgoing_combat_prevention_rejects_player_and_noncreature_target_filters() {
    for kind in ["AnyPlayer", "AnyPermanent"] {
        let ron = format!("(id: \"test\", name: \"Test\", face_id: \"test\", mana_cost: \"{{W}}\", types: [\"Instant\"], spell_effect: [PreventAllCombatDamageByTargetTurn(target: (kind: {kind}))])");
        assert!(matches!(CardRegistry::from_chunks_and_tokens(&[&ron], &[]),
            Err(tricerules_cards::registry::RegistryError::InvalidCard { reason, .. }) if reason.contains("creature target filter")));
    }
}
