use tricerules_cards::primitives::{Amount, PlayerRecipient, SpellEffectKind, TriggerCondition};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

#[test]
fn ichor_wellspring_registers_one_alternative_event_ability() {
    let registry = CardRegistry::global();
    let card = registry
        .get("ichor_wellspring")
        .expect("Ichor Wellspring registry definition");

    assert_eq!(card.name, "Ichor Wellspring");
    assert_eq!(
        registry.id_for_name("Ichor Wellspring"),
        Some("ichor_wellspring")
    );
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "ichor_wellspring");
    assert_eq!(face.mana_cost.to_string(), "{2}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.colors().is_empty());
    assert!(face.keywords.is_empty());
    assert!(face.spell_effect.is_empty());
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());
    assert_eq!(face.triggered_abilities.len(), 1);

    let ability = &face.triggered_abilities[0];
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::WhenSelfEntersOrIsPutIntoGraveyardFromBattlefield
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );
    assert_eq!(
        ability.fallback_text("Ichor Wellspring"),
        "When Ichor Wellspring enters or is put into a graveyard from the battlefield, draw a card."
    );
}
