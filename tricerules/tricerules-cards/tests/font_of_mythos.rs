use tricerules_cards::primitives::{Amount, PlayerRecipient, SpellEffectKind, TriggerCondition};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

#[test]
fn font_of_mythos_registry_matches_reviewed_definition() {
    let card = CardRegistry::global()
        .get("font_of_mythos")
        .expect("Font of Mythos is registered");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.name, "Font of Mythos");

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "font_of_mythos");
    assert_eq!(face.mana_cost.to_string(), "{4}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.power.is_none() && face.toughness.is_none());

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Font of Mythos has one triggered ability");
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        trigger.trigger,
        TriggerCondition::AtBeginningOfDrawStep { .. }
    ));
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::Draw {
            who: PlayerRecipient::AffectedPlayer,
            count: Amount::Fixed(2),
        }]
    ));
}
