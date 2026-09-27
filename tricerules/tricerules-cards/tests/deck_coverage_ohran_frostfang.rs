use tricerules_cards::primitives::{
    CastTriggerPlayer, CreatureScopeController, PlayerRecipient, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, Color, Keyword, Layout, TriggerCondition,
};

#[test]
fn ohran_frostfang_registers_its_exact_characteristics_and_typed_abilities() {
    let registry = CardRegistry::global();
    assert_eq!(
        registry.id_for_name("Ohran Frostfang"),
        Some("ohran_frostfang")
    );
    let card = registry.get("ohran_frostfang").expect("Ohran Frostfang");
    assert_eq!(card.name, "Ohran Frostfang");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "ohran_frostfang");
    assert_eq!(face.mana_cost.to_string(), "{3}{G}{G}");
    assert_eq!(face.supertypes, ["Snow"]);
    assert_eq!(face.types, ["Creature", "Snake"]);
    assert_eq!(face.colors(), [Color::Green]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(6)));
    assert!(
        face.keywords.is_empty(),
        "Deathtouch is granted only while attacking"
    );
    assert!(face.spell_effect.is_empty());
    assert!(face.activated_abilities.is_empty());

    let [anthem] = face.static_abilities.as_slice() else {
        panic!("Ohran Frostfang has exactly one static ability");
    };
    assert!(matches!(
        &anthem.presentation,
        AbilityPresentation::OracleLines(lines) if lines == &[1]
    ));
    assert!(matches!(
        &anthem.definition,
        StaticAbilityDef::AnthemKeyword {
            filter,
            condition: None,
            keyword: Keyword::Deathtouch,
        } if filter.controller == Some(CreatureScopeController::YouControl)
            && filter.attacking
            && filter.subtype.is_none()
            && filter.color.is_none()
            && filter.required_keyword.is_none()
            && !filter.exclude_self
    ));

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Ohran Frostfang has exactly one triggered ability");
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert!(matches!(
        &trigger.presentation,
        AbilityPresentation::OracleLines(lines) if lines == &[2]
    ));
    assert_eq!(
        trigger.trigger,
        TriggerCondition::WheneverCreatureDealsCombatDamageToPlayer {
            source_controller: CastTriggerPlayer::Controller,
            damaged_player: CastTriggerPlayer::AnyPlayer,
        }
    );
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::Draw {
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
        }]
    ));
}
