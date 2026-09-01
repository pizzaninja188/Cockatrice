use tricerules_cards::primitives::CardTypeFilter;
use tricerules_cards::{
    AbilityCost, AbilityPresentation, CardFace, CardRegistry, Keyword, ManaAmount,
    ManaSpendingRestriction, SpellEffectKind,
};

fn mana_restriction(face: &CardFace, expected: ManaAmount) -> &ManaSpendingRestriction {
    let ability = &face.activated_abilities[0];
    assert_eq!(ability.costs, [AbilityCost::Tap]);
    let [SpellEffectKind::ProduceMana {
        options,
        restriction: Some(restriction),
        conditional: None,
    }] = ability.effect.as_slice()
    else {
        panic!("expected one restricted mana effect")
    };
    assert_eq!(options, &[expected]);
    restriction
}

#[test]
fn issue_190_cards_are_authored_with_exact_characteristics() {
    let registry = CardRegistry::global();

    let punks = registry
        .get("purple_dragon_punks")
        .expect("Purple Dragon Punks");
    let punks = punks.primary_face();
    assert_eq!(punks.mana_cost.to_string(), "{1}{R}");
    assert_eq!(punks.types, ["Creature", "Human", "Rogue"]);
    assert_eq!((punks.power, punks.toughness), (Some(2), Some(2)));
    assert!(punks.keywords.is_empty());
    assert_eq!(punks.activated_abilities.len(), 1);
    assert_eq!(
        punks.activated_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );

    let helper = registry.get("hydraulic_helper").expect("Hydraulic Helper");
    let helper = helper.primary_face();
    assert_eq!(helper.mana_cost.to_string(), "{1}{U}");
    assert_eq!(helper.types, ["Artifact", "Creature", "Robot"]);
    assert_eq!((helper.power, helper.toughness), (Some(2), Some(3)));
    assert_eq!(helper.keywords, [Keyword::Defender]);
    assert_eq!(helper.activated_abilities.len(), 1);
    assert_eq!(
        helper.activated_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );

    let punks_restriction = mana_restriction(
        punks,
        ManaAmount {
            r: 1,
            ..Default::default()
        },
    );
    assert_eq!(punks_restriction.cast_spell.len(), 1);
    assert_eq!(
        punks_restriction.cast_spell[0].card_type,
        Some(CardTypeFilter::Artifact)
    );
    assert!(punks_restriction.activate_any_ability);
    assert!(!punks_restriction.all_nonspell_costs);
    assert!(punks_restriction.activate_ability.is_empty());
    assert!(punks_restriction.special_actions.is_empty());

    let helper_restriction = mana_restriction(
        helper,
        ManaAmount {
            u: 1,
            ..Default::default()
        },
    );
    assert_eq!(helper_restriction.cast_spell.len(), 1);
    assert_eq!(
        helper_restriction.cast_spell[0].card_type,
        Some(CardTypeFilter::Artifact)
    );
    assert!(!helper_restriction.activate_any_ability);
    assert!(helper_restriction.all_nonspell_costs);
    assert!(helper_restriction.activate_ability.is_empty());
    assert!(helper_restriction.special_actions.is_empty());
}
