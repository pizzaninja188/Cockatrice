use tricerules_cards::primitives::{DrawReplacementCondition, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, CardRegistry, Color};

#[test]
fn thought_reflection_exact_printed_identity_and_mandatory_controller_draw_replacement() {
    let card = CardRegistry::global().get("thought_reflection").unwrap();
    let face = card.primary_face();
    assert_eq!(card.name, "Thought Reflection");
    assert_eq!(face.mana_cost.to_string(), "{4}{U}{U}{U}");
    assert_eq!(face.types, ["Enchantment"]);
    assert!(face.supertypes.is_empty());
    assert_eq!(face.colors(), [Color::Blue]);
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(face.static_abilities[0].ability_id.as_str(), "static_01");
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        face.static_abilities[0].definition,
        StaticAbilityDef::DoubleControllerDraws {
            condition: DrawReplacementCondition::Always
        }
    );
}

#[test]
fn insight_exact_legendary_identity_and_first_successful_own_draw_step_exception() {
    let card = CardRegistry::global()
        .get("teferis_ageless_insight")
        .unwrap();
    let face = card.primary_face();
    assert_eq!(card.name, "Teferi's Ageless Insight");
    assert_eq!(face.mana_cost.to_string(), "{2}{U}{U}");
    assert_eq!(face.types, ["Enchantment"]);
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.colors(), [Color::Blue]);
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(face.static_abilities[0].ability_id.as_str(), "static_01");
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        face.static_abilities[0].definition,
        StaticAbilityDef::DoubleControllerDraws {
            condition: DrawReplacementCondition::ExceptFirstSuccessfulDrawInOwnDrawStep
        }
    );
}
