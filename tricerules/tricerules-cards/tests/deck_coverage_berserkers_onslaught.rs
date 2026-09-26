use tricerules_cards::primitives::{CreatureScopeController, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, CardRegistry, Color, Keyword, Layout};

#[test]
fn berserkers_onslaught_registers_its_attacking_creature_double_strike_grant() {
    let registry = CardRegistry::global();
    assert_eq!(
        registry.id_for_name("Berserkers' Onslaught"),
        Some("berserkers_onslaught")
    );
    let card = registry
        .get("berserkers_onslaught")
        .expect("Berserkers' Onslaught");
    assert_eq!(card.name, "Berserkers' Onslaught");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "berserkers_onslaught");
    assert_eq!(face.mana_cost.to_string(), "{3}{R}{R}");
    assert_eq!(face.types, ["Enchantment"]);
    assert_eq!(face.colors(), [Color::Red]);
    assert!(face.spell_effect.is_empty());
    assert!(face.activated_abilities.is_empty());
    assert!(face.triggered_abilities.is_empty());

    let [ability] = face.static_abilities.as_slice() else {
        panic!("Berserkers' Onslaught has exactly one static ability");
    };
    assert!(matches!(
        &ability.presentation,
        AbilityPresentation::OracleLines(lines) if lines == &[1]
    ));
    assert!(matches!(
        &ability.definition,
        StaticAbilityDef::AnthemKeyword {
            filter,
            condition: None,
            keyword: Keyword::DoubleStrike,
        } if filter.controller == Some(CreatureScopeController::YouControl)
            && filter.attacking
            && filter.subtype.is_none()
            && !filter.exclude_self
    ));
}
