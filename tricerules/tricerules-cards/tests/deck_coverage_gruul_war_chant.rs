use tricerules_cards::primitives::{CreatureScopeController, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, CardRegistry, Color, Keyword, Layout};

#[test]
fn gruul_war_chant_registers_its_attack_scoped_pump_and_menace_effects() {
    let registry = CardRegistry::global();
    assert_eq!(
        registry.id_for_name("Gruul War Chant"),
        Some("gruul_war_chant")
    );
    let card = registry.get("gruul_war_chant").expect("Gruul War Chant");
    assert_eq!(card.name, "Gruul War Chant");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "gruul_war_chant");
    assert_eq!(face.mana_cost.to_string(), "{2}{R}{G}");
    assert_eq!(face.types, ["Enchantment"]);
    assert!(face.colors().contains(&Color::Red));
    assert!(face.colors().contains(&Color::Green));
    assert!(face.spell_effect.is_empty());
    assert!(face.activated_abilities.is_empty());
    assert!(face.triggered_abilities.is_empty());

    let [anthem, menace] = face.static_abilities.as_slice() else {
        panic!("Gruul War Chant has exactly two static effects");
    };
    for ability in [anthem, menace] {
        assert!(matches!(
            &ability.presentation,
            AbilityPresentation::OracleLines(lines) if lines == &[1]
        ));
    }
    assert!(matches!(
        &anthem.definition,
        StaticAbilityDef::AnthemPt {
            filter,
            condition: None,
            delta_power: 1,
            delta_toughness: 0,
        } if filter.controller == Some(CreatureScopeController::YouControl)
            && filter.attacking
            && filter.subtype.is_none()
            && !filter.exclude_self
    ));
    assert!(matches!(
        &menace.definition,
        StaticAbilityDef::AnthemKeyword {
            filter,
            condition: None,
            keyword: Keyword::Menace,
        } if filter.controller == Some(CreatureScopeController::YouControl)
            && filter.attacking
            && filter.subtype.is_none()
            && !filter.exclude_self
    ));
}
