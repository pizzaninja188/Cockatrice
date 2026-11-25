use tricerules_cards::{Amount, CountExpression, Keyword, RelativePlayerSet, SpellCostModifier};

#[test]
fn ghalta_exact_printed_definition_and_generic_only_current_creature_power_cost() {
    let card = tricerules_cards::registry::global()
        .get("ghalta,_primal_hunger")
        .unwrap();
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
    assert!(filter.matching.kind == tricerules_cards::primitives::TargetKind::Creature);
    assert!(
        filter.any_of.is_none()
            && filter.matching.token.is_none()
            && filter.matching.is_color.is_none()
            && filter.matching.name.is_none()
    );
    assert!(filter.matching.required_subtypes.is_empty() && !filter.exclude_source);
}
