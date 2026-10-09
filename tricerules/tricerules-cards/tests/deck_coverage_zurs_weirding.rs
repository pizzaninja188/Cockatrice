use tricerules_cards::primitives::StaticAbilityDef;
use tricerules_cards::{AbilityPresentation, Color};

#[test]
fn zurs_weirding_has_its_exact_printed_identity() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("zurs_weirding")
        .expect("Zur's Weirding is implemented in the card registry");
    assert_eq!(
        registry.id_for_name("Zur's Weirding"),
        Some("zurs_weirding")
    );
    assert_eq!(card.name, "Zur's Weirding");
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "zurs_weirding");
    assert_eq!(face.mana_cost.to_string(), "{3}{U}");
    assert_eq!(face.types, ["Enchantment"]);
    assert!(face.supertypes.is_empty());
    assert_eq!(face.colors(), [Color::Blue]);
    assert!(face.spell_effect.is_empty());
    let [ability] = face.static_abilities.as_slice() else {
        panic!("one static ability");
    };
    assert_eq!(ability.ability_id.as_str(), "static_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1, 2])
    );
    assert_eq!(ability.definition, StaticAbilityDef::ZurWeirding);
}
