use tricerules_cards::primitives::{DamageDoublingSubject, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Color, Layout};

#[test]
fn gratuitous_violence_is_registered_as_a_complete_card() {
    let card = tricerules_cards::registry::global()
        .get("gratuitous_violence")
        .expect("Gratuitous Violence must be a complete ruled-card definition");
    assert_eq!(card.name, "Gratuitous Violence");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "gratuitous_violence");
    assert_eq!(face.mana_cost.to_string(), "{2}{R}{R}{R}");
    assert_eq!(face.types, ["Enchantment"]);
    assert_eq!(face.colors(), [Color::Red]);
    assert!(face.power.is_none() && face.toughness.is_none());
    assert!(face.spell_effect.is_empty());
    assert!(face.activated_abilities.is_empty() && face.triggered_abilities.is_empty());
    let [ability] = face.static_abilities.as_slice() else {
        panic!("Gratuitous Violence has exactly one static ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.definition,
        StaticAbilityDef::DoubleDamage {
            subject: DamageDoublingSubject::CreatureYouControl,
        }
    );
}
