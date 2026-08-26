use tricerules_cards::primitives::{
    CardTypeFilter, DiscardChooser, EffectSubject, GameCondition, PlayerRecipient,
    RelativePlayerSet, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{CardRegistry, Keyword, TriggerCondition};

#[test]
fn issue_115_cards_and_treasure_have_exact_shared_shapes() {
    let registry = CardRegistry::global();

    let pick = registry.get("goldvein_pick").expect("Goldvein Pick");
    assert!(pick.partial.is_none());
    let pick_face = pick.primary_face();
    assert_eq!(pick_face.mana_cost.to_string(), "{2}");
    assert!(matches!(
        pick_face.static_abilities.as_slice(),
        [StaticAbilityDef::AttachedModifier {
            condition: None,
            delta_power: 1,
            delta_toughness: 1,
            ..
        }]
    ));
    assert!(matches!(
        pick_face.triggered_abilities.as_slice(),
        [ability]
            if ability.trigger
                == TriggerCondition::WheneverAttachedObjectDealsCombatDamageToPlayer
                && matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::CreateTokens {
                        token, count, who, ..
                    }]
                        if token == "treasure"
                            && *count == tricerules_cards::Amount::Fixed(1)
                            && *who == PlayerRecipient::SourceController
                )
    ));

    let skull = registry.get("cracked_skull").expect("Cracked Skull");
    assert!(skull.partial.is_none());
    let skull_face = skull.primary_face();
    assert_eq!(skull_face.mana_cost.to_string(), "{2}{B}");
    assert!(matches!(
        skull_face.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::DiscardCards {
            count: 1,
            chooser: DiscardChooser::Controller,
            card_filter: Some(CardTypeFilter::Nonland),
            optional: true,
            ..
        }]
    ));
    assert_eq!(
        skull_face.triggered_abilities[1].trigger,
        TriggerCondition::WheneverAttachedObjectIsDealtDamage
    );
    assert_eq!(
        skull_face.triggered_abilities[1].effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::TriggerObject,
        }]
    );

    let katana = registry
        .get("quick-draw_katana")
        .expect("Quick-Draw Katana");
    assert!(katana.partial.is_none());
    assert!(matches!(
        katana.primary_face().static_abilities.as_slice(),
        [StaticAbilityDef::AttachedModifier {
            condition: Some(GameCondition::ActivePlayer {
                players: RelativePlayerSet::Controller,
            }),
            delta_power: 2,
            delta_toughness: 0,
            keywords,
            ..
        }] if *keywords == [Keyword::FirstStrike]
    ));

    let treasure = registry.get("treasure").expect("Treasure token");
    assert!(registry.is_token("treasure"));
    let treasure_face = treasure.primary_face();
    assert!(treasure_face.is_artifact);
    assert!(treasure_face
        .types
        .iter()
        .any(|card_type| card_type == "Treasure"));
    assert_eq!(treasure_face.activated_abilities.len(), 1);
}
