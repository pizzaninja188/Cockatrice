//! Exact Propaganda card definition and static attack-cost mapping.

use tricerules_cards::primitives::StaticAbilityDef;
use tricerules_cards::{AbilityPresentation, CardRegistry, ManaCost};

#[test]
fn propaganda_registers_its_complete_card_definition() {
    let face = CardRegistry::global()
        .get("propaganda")
        .expect("complete Propaganda definition")
        .primary_face();

    assert_eq!(face.mana_cost, ManaCost::parse("{2}{U}").unwrap());
    assert_eq!(face.types, vec!["Enchantment".to_string()]);
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(face.static_abilities[0].ability_id.as_str(), "static_01");
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let StaticAbilityDef::AttackTax {
        generic_per_attacker,
    } = &face.static_abilities[0].definition
    else {
        panic!("Propaganda carries its typed attack tax");
    };
    assert_eq!(*generic_per_attacker, 2);
}
