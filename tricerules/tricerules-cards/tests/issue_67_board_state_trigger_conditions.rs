use tricerules_cards::primitives::{
    BattlefieldAggregate, BattlefieldPermanentFilter, CardTypeFilter, GameCondition, InterveningIf,
    PlayerRecipient, RelativePlayerSet, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{CardRegistry, Keyword};

fn face(card_id: &str) -> &'static tricerules_cards::CardFace {
    let definition = CardRegistry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("{card_id} must be registered"));
    assert!(definition.partial.is_none());
    definition.primary_face()
}

fn battlefield_condition(card_id: &str) -> &GameCondition {
    let ability = &face(card_id).triggered_abilities[0];
    let Some(InterveningIf::GameCondition(condition)) = ability.intervening_if.as_ref() else {
        panic!("{card_id} must use the shared game condition")
    };
    condition
}

#[test]
fn issue_67_cards_use_one_validated_battlefield_condition_vocabulary() {
    for card_id in [
        "scholar_of_stars",
        "faerie_miscreant",
        "ornery_dilophosaur",
        "turret_ogre",
    ] {
        assert!(battlefield_condition(card_id).validate().is_ok());
    }

    assert!(matches!(
        battlefield_condition("scholar_of_stars"),
        GameCondition::BattlefieldAggregate {
            filter: BattlefieldPermanentFilter {
                controllers: RelativePlayerSet::Controller,
                card_type: Some(CardTypeFilter::Artifact),
                color: None,
                name: None,
                exclude_source: false,
                ..
            },
            aggregate: BattlefieldAggregate::Count,
            min: Some(1),
            max: None,
        }
    ));
    assert!(matches!(
        battlefield_condition("faerie_miscreant"),
        GameCondition::BattlefieldAggregate {
            filter: BattlefieldPermanentFilter {
                card_type: Some(CardTypeFilter::Creature),
                name: Some(name),
                exclude_source: true,
                ..
            },
            aggregate: BattlefieldAggregate::Count,
            min: Some(1),
            max: None,
        } if name == "Faerie Miscreant"
    ));
    assert!(matches!(
        battlefield_condition("ornery_dilophosaur"),
        GameCondition::BattlefieldAggregate {
            filter: BattlefieldPermanentFilter {
                exclude_source: false,
                ..
            },
            aggregate: BattlefieldAggregate::MaximumPower,
            min: Some(4),
            max: None,
        }
    ));
    assert!(matches!(
        battlefield_condition("turret_ogre"),
        GameCondition::BattlefieldAggregate {
            filter: BattlefieldPermanentFilter {
                exclude_source: true,
                ..
            },
            aggregate: BattlefieldAggregate::MaximumPower,
            min: Some(4),
            max: None,
        }
    ));
}

#[test]
fn issue_67_card_characteristics_and_effects_match_oracle() {
    let scholar = face("scholar_of_stars");
    assert_eq!((scholar.power, scholar.toughness), (Some(3), Some(2)));
    assert_eq!(
        scholar.triggered_abilities[0].trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    assert!(matches!(
        scholar.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::Draw { .. }]
    ));

    let faerie = face("faerie_miscreant");
    assert_eq!((faerie.power, faerie.toughness), (Some(1), Some(1)));
    assert!(faerie.keywords.contains(&Keyword::Flying));

    let dinosaur = face("ornery_dilophosaur");
    assert_eq!((dinosaur.power, dinosaur.toughness), (Some(2), Some(2)));
    assert!(dinosaur.keywords.contains(&Keyword::Deathtouch));
    assert_eq!(
        dinosaur.triggered_abilities[0].trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );

    let ogre = face("turret_ogre");
    assert_eq!((ogre.power, ogre.toughness), (Some(4), Some(3)));
    assert!(ogre.keywords.contains(&Keyword::Reach));
    assert!(matches!(
        ogre.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::DamagePlayer {
            who: PlayerRecipient::EachOpponent,
            ..
        }]
    ));
}

#[test]
fn battlefield_aggregate_conditions_reject_invalid_bounds_and_names() {
    let condition = |name: Option<&str>, min, max| GameCondition::BattlefieldAggregate {
        filter: BattlefieldPermanentFilter {
            any_of: None,
            controllers: RelativePlayerSet::Controller,
            card_type: None,
            color: None,
            name: name.map(str::to_string),
            required_subtypes: vec![],
            exclude_source: false,
        },
        aggregate: BattlefieldAggregate::TotalPower,
        min,
        max,
    };
    assert!(condition(None, None, None).validate().is_err());
    assert!(condition(None, Some(2), Some(1)).validate().is_err());
    assert!(condition(Some("  "), Some(1), None).validate().is_err());
    assert!(condition(None, Some(1), None).validate().is_ok());
}
