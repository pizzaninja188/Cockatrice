use tricerules_cards::primitives::{CountExpression, ManaCostChoiceKind, StaticAbilityDef};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, CounterKind, Layout, ManaAmount, SpellEffectKind,
};

#[test]
fn everflowing_chalice_registers_its_complete_typed_face() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("everflowing_chalice")
        .expect("complete Chalice definition");
    assert_eq!(card.name, "Everflowing Chalice");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "everflowing_chalice");
    assert_eq!(face.mana_cost.to_string(), "{0}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.colors().is_empty());
    assert!(face.power.is_none() && face.toughness.is_none());
    assert!(face.spell_effect.is_empty());
    assert_eq!(face.cast_cost_groups.len(), 1);
    let group = &face.cast_cost_groups[0];
    assert_eq!((group.min, group.max), (0, 1));
    assert_eq!(group.options.len(), 1);
    assert_eq!(
        group.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let tricerules_cards::CastCostOptionDef::Mana {
        option_id,
        kind,
        cost,
        presentation,
    } = &group.options[0]
    else {
        panic!("typed repeated fee");
    };
    assert_eq!(*kind, ManaCostChoiceKind::Multikicker);
    assert_eq!(cost.to_string(), "{2}");
    assert_eq!(*presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    let StaticAbilityDef::EntersWithCounters {
        counter, amount, ..
    } = &face.static_abilities[0].definition
    else {
        panic!("intrinsic entry count");
    };
    assert_eq!(*counter, CounterKind::Charge);
    let tricerules_cards::Amount::Count(CountExpression::CastCostPaymentCount { cost: link }) =
        amount
    else {
        panic!("linked payment count");
    };
    assert_eq!(link.group_id, group.group_id);
    assert_eq!(&link.option_id, option_id);
    assert_eq!(face.activated_abilities.len(), 1);
    let mana = &face.activated_abilities[0];
    assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![3]));
    assert_eq!(mana.costs, [AbilityCost::Tap]);
    assert_eq!(
        mana.effect,
        [SpellEffectKind::ProduceManaPerSourceCounter {
            counter: CounterKind::Charge,
            options: vec![ManaAmount {
                c: 1,
                ..Default::default()
            }],
        }]
    );
}
