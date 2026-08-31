use tricerules_cards::primitives::{
    AbilityCost, CounterKind, CreatureScopeController, CreatureScopeFilter, EffectSubject, Keyword,
    SpellEffectKind, StaticAbilityDef, TargetController, TargetKind, TriggerCondition,
};
use tricerules_cards::CardRegistry;

#[test]
fn issue_79_cards_use_shared_name_and_counter_scope_predicates() {
    let registry = CardRegistry::global();

    let mastiff = registry
        .get("pack_mastiff")
        .expect("Pack Mastiff must be registered");
    let mastiff_face = mastiff.primary_face();
    assert_eq!(mastiff_face.mana_cost.to_string(), "{1}{R}");
    assert_eq!(mastiff_face.types, ["Creature", "Dog"]);
    assert_eq!(
        (mastiff_face.power, mastiff_face.toughness),
        (Some(2), Some(2))
    );
    assert!(matches!(
        mastiff_face.activated_abilities[0].costs.as_slice(),
        [AbilityCost::Mana(cost)] if cost.to_string() == "{1}{R}"
    ));
    assert_eq!(
        mastiff_face.activated_abilities[0].effect,
        [SpellEffectKind::PumpAll {
            filter: CreatureScopeFilter {
                controller: Some(CreatureScopeController::YouControl),
                name: Some("Pack Mastiff".to_string()),
                ..CreatureScopeFilter::default()
            },
            power: 1,
            toughness: 0,
        }]
    );

    let pridemalkin = registry
        .get("pridemalkin")
        .expect("Pridemalkin must be registered");
    let pride_face = pridemalkin.primary_face();
    assert_eq!(pride_face.mana_cost.to_string(), "{2}{G}");
    assert_eq!(pride_face.types, ["Creature", "Cat"]);
    assert_eq!((pride_face.power, pride_face.toughness), (Some(2), Some(1)));
    assert_eq!(
        pride_face.triggered_abilities[0].trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    assert!(matches!(
        pride_face.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: 1,
            subject: EffectSubject::Chosen(target),
        }] if target.kind == TargetKind::Creature && target.controller == TargetController::You
    ));
    assert_eq!(
        pride_face.static_abilities,
        [StaticAbilityDef::AnthemKeyword {
            filter: CreatureScopeFilter {
                controller: Some(CreatureScopeController::YouControl),
                required_counter: Some(CounterKind::PlusOnePlusOne),
                ..CreatureScopeFilter::default()
            },
            condition: None,
            keyword: Keyword::Trample,
        }]
    );
}

#[test]
fn creature_scope_filter_rejects_blank_names() {
    assert!(CreatureScopeFilter {
        name: Some("  ".to_string()),
        ..CreatureScopeFilter::default()
    }
    .validate()
    .is_err());
    assert!(CreatureScopeFilter::default().validate().is_ok());
}
