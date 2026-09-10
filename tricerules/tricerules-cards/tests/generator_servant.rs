use tricerules_cards::primitives::{CardTypeFilter, ManaSpendingEffect};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, CardRegistry, Keyword, ManaAmount, SpellEffectKind,
};

#[test]
fn generator_servant_is_authored_with_exact_characteristics_and_mana_rule() {
    let card = CardRegistry::global()
        .get("generator_servant")
        .expect("Generator Servant");
    let face = card.primary_face();
    assert_eq!(card.name, "Generator Servant");
    assert_eq!(face.mana_cost.to_string(), "{1}{R}");
    assert_eq!(face.types, ["Creature", "Elemental"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(1)));
    assert_eq!(face.activated_abilities.len(), 1);

    let ability = &face.activated_abilities[0];
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.costs,
        [AbilityCost::Tap, AbilityCost::SacrificeSelf]
    );
    let [SpellEffectKind::ProduceMana {
        options,
        restriction: Some(rule),
        conditional: None,
    }] = ability.effect.as_slice()
    else {
        panic!("expected one tagged mana effect")
    };
    assert_eq!(
        options,
        &[ManaAmount {
            c: 2,
            ..Default::default()
        }]
    );
    assert!(rule.unrestricted);
    assert!(rule.cast_spell.is_empty());
    assert!(rule.activate_ability.is_empty());
    assert!(!rule.activate_any_ability);
    assert!(!rule.all_nonspell_costs);
    assert!(rule.special_actions.is_empty());
    assert_eq!(rule.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(
        rule.spending_effects,
        [ManaSpendingEffect::GrantKeywordsToSpellUntilEndOfTurn {
            filter: tricerules_cards::ManaSpendFilter {
                card_type: Some(CardTypeFilter::Creature),
                subtype: None,
            },
            keywords: vec![Keyword::Haste],
        }]
    );
}
