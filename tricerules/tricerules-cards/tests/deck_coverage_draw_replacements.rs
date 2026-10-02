use tricerules_cards::primitives::{
    AbilityCost, Amount, DrawReplacementCondition, SpellEffectKind, StaticAbilityDef, TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Color, GameCondition};

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

#[test]
fn laboratory_maniac_exact_identity_and_empty_library_replacement() {
    let registry = CardRegistry::global();
    let card = registry.get("laboratory_maniac").unwrap();
    assert_eq!(
        registry.id_for_name("Laboratory Maniac"),
        Some("laboratory_maniac")
    );
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{U}");
    assert_eq!(face.types, ["Creature", "Human", "Wizard"]);
    assert_eq!(face.colors(), [Color::Blue]);
    assert_eq!(face.power, Some(2));
    assert_eq!(face.toughness, Some(2));
    assert!(face.activated_abilities.is_empty());
    let [ability] = face.static_abilities.as_slice() else {
        panic!("one static");
    };
    assert_eq!(ability.ability_id.as_str(), "static_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.definition,
        StaticAbilityDef::WinControllerInsteadOfEmptyLibraryDraw
    );
}

#[test]
fn jace_wielder_exact_identity_loyalty_target_mill_draw_and_final_win() {
    let registry = CardRegistry::global();
    let card = registry.get("jace,_wielder_of_mysteries").unwrap();
    assert_eq!(
        registry.id_for_name("Jace, Wielder of Mysteries"),
        Some("jace,_wielder_of_mysteries")
    );
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "jace_wielder_of_mysteries");
    assert_eq!(face.mana_cost.to_string(), "{1}{U}{U}{U}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Jace"]);
    assert_eq!(face.colors(), [Color::Blue]);
    assert_eq!(face.loyalty, Some(4));
    let static_ability = face
        .static_abilities
        .iter()
        .find(|ability| ability.ability_id.as_str() == "static_01")
        .unwrap();
    assert_eq!(
        static_ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        static_ability.definition,
        StaticAbilityDef::WinControllerInsteadOfEmptyLibraryDraw
    );
    let [plus, minus] = face.activated_abilities.as_slice() else {
        panic!("two loyalty abilities");
    };
    assert_eq!(plus.costs, [AbilityCost::Loyalty(1)]);
    assert_eq!(plus.presentation, AbilityPresentation::OracleLines(vec![2]));
    assert!(
        matches!(plus.effect.as_slice(), [SpellEffectKind::MillTargetPlayer { count: 2, target }, SpellEffectKind::Draw { count: Amount::Fixed(1), .. }] if target.kind == TargetKind::AnyPlayer)
    );
    assert_eq!(minus.costs, [AbilityCost::Loyalty(-8)]);
    assert_eq!(
        minus.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert!(matches!(
        minus.effect.as_slice(),
        [
            SpellEffectKind::Draw {
                count: Amount::Fixed(7),
                ..
            },
            SpellEffectKind::WinGameIf {
                condition: GameCondition::ControllerLibraryEmpty
            }
        ]
    ));
}
