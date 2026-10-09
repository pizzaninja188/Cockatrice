use tricerules_cards::primitives::{DrawReplacementCondition, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Layout};

#[test]
fn archive_registers_complete_two_clause_legendary_artifact() {
    let card = tricerules_cards::registry::global()
        .get("alhammarrets_archive")
        .unwrap();
    assert_eq!(card.name, "Alhammarret's Archive");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "alhammarrets_archive");
    assert_eq!(face.mana_cost.to_string(), "{5}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.colors().is_empty());
    assert!(face.power.is_none() && face.toughness.is_none());
    assert!(face.spell_effect.is_empty());
    assert!(face.activated_abilities.is_empty() && face.triggered_abilities.is_empty());
    assert_eq!(face.static_abilities.len(), 2);
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        face.static_abilities[0].definition,
        StaticAbilityDef::DoubleControllerLifeGain
    );
    assert_eq!(
        face.static_abilities[1].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        face.static_abilities[1].definition,
        StaticAbilityDef::DoubleControllerDraws {
            condition: DrawReplacementCondition::ExceptFirstSuccessfulDrawInOwnDrawStep,
        }
    );
}
