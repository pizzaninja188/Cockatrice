use tricerules_cards::primitives::CardTypeFilter;
use tricerules_cards::{
    Amount, CardRegistry, CountExpression, Keyword, RelativePlayerSet, SpellCostModifier,
};

#[test]
fn ghalta_exact_printed_definition_and_generic_only_current_creature_power_cost() {
    let card = CardRegistry::global().get("ghalta,_primal_hunger").unwrap();
    let face = card.primary_face();
    assert_eq!(card.name, "Ghalta, Primal Hunger");
    assert_eq!(face.mana_cost.to_string(), "{10}{G}{G}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Creature", "Elder", "Dinosaur"]);
    assert_eq!((face.power, face.toughness), (Some(12), Some(12)));
    assert_eq!(face.keywords, [Keyword::Trample]);
    assert_eq!(face.cost_modifiers.len(), 1);
    let SpellCostModifier::GenericReduction {
        amount: Amount::Count(CountExpression::BattlefieldPowerSum { filter }),
    } = &face.cost_modifiers[0]
    else {
        panic!("exact generic reduction expression required")
    };
    assert_eq!(filter.controllers, RelativePlayerSet::Controller);
    assert_eq!(filter.card_type, Some(CardTypeFilter::Creature));
    assert!(
        filter.any_of.is_none()
            && filter.token.is_none()
            && filter.color.is_none()
            && filter.name.is_none()
    );
    assert!(filter.required_subtypes.is_empty() && !filter.exclude_source);
}
