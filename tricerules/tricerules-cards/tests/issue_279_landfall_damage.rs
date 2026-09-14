use tricerules_cards::primitives::{
    CastTriggerPlayer, PermanentEventFilter, PermanentTypeFilter, PlayerRecipient,
};
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, Keyword, SpellEffectKind, TriggerCondition,
};

#[test]
fn issue_279_registers_exact_landfall_damage_cohort() {
    let registry = CardRegistry::global();

    let sabotender = registry
        .get("sabotender")
        .expect("Sabotender should be generated")
        .primary_face();
    assert_eq!(sabotender.mana_cost.to_string(), "{1}{R}");
    assert_eq!(sabotender.types, ["Creature", "Plant"]);
    assert_eq!((sabotender.power, sabotender.toughness), (Some(2), Some(1)));
    assert_eq!(sabotender.keywords, [Keyword::Reach]);

    let spitfire = registry
        .get("spitfire_lagac")
        .expect("Spitfire Lagac should be generated")
        .primary_face();
    assert_eq!(spitfire.mana_cost.to_string(), "{3}{R}");
    assert_eq!(spitfire.types, ["Creature", "Lizard"]);
    assert_eq!((spitfire.power, spitfire.toughness), (Some(3), Some(4)));
    assert!(spitfire.keywords.is_empty());

    for (face, oracle_line) in [(sabotender, 2), (spitfire, 1)] {
        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("expected one Landfall damage ability")
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverPermanentEntersBattlefield {
                controller: CastTriggerPlayer::Controller,
                filter: PermanentEventFilter {
                    permanent_type: Some(PermanentTypeFilter::Land),
                    ..Default::default()
                },
                creature_filter: None,
            }
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::DamagePlayer {
                amount: Amount::Fixed(1),
                who: PlayerRecipient::EachOpponent,
            }]
        );
    }
}
