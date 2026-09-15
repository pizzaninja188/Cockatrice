use tricerules_cards::primitives::{PlayerRecipient, SpellEffectKind, TriggerCondition};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, ActivationTiming, Amount, CardRegistry, Keyword, ManaCost,
};

fn assert_food_leave_trigger(face: &tricerules_cards::CardFace, card_id: &str) {
    assert!(
        face.spell_effect.is_empty(),
        "{card_id} must not be a spell"
    );
    assert!(
        face.activated_abilities.is_empty(),
        "{card_id} has no activated abilities"
    );
    assert!(
        face.static_abilities.is_empty(),
        "{card_id} has no static abilities"
    );
    assert!(
        face.characteristic_defining_abilities.is_empty(),
        "{card_id} has no characteristic ability"
    );
    assert_eq!(
        face.keywords,
        [Keyword::Flying],
        "{card_id} keyword surface"
    );
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("{card_id} should have exactly one triggered ability");
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        trigger.trigger,
        TriggerCondition::WhenSelfLeavesBattlefield,
        "{card_id} trigger"
    );
    assert!(!trigger.may);
    assert!(trigger.modal.is_none());
    assert!(trigger.targeting.is_none());
    assert!(trigger.intervening_if.is_none());
    assert!(!trigger.triggers_only_once);
    assert!(trigger.max_triggers_per_turn.is_none());
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::CreateTokens {
            token: "food".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }],
        "{card_id} effect"
    );
}

#[test]
fn issue_297_city_pigeon_has_exact_generated_leave_food_shape() {
    let registry = CardRegistry::global();
    let city = registry
        .get("city_pigeon")
        .expect("City Pigeon must be generated");
    assert_eq!(city.name, "City Pigeon");
    assert_eq!(registry.id_for_name("City Pigeon"), Some("city_pigeon"));
    let face = city.primary_face();
    assert_eq!(face.face_id.as_str(), "city_pigeon");
    assert_eq!(face.mana_cost.to_string(), "{W}");
    assert_eq!(face.types, ["Creature", "Bird"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(1)));
    assert_food_leave_trigger(face, "city_pigeon");
}

#[test]
fn issue_297_featherbrained_filcher_calibrates_without_duplicate_generation() {
    let registry = CardRegistry::global();
    let feather = registry
        .get("featherbrained_filcher")
        .expect("existing Featherbrained Filcher calibration");
    assert_eq!(feather.name, "Featherbrained Filcher");
    let face = feather.primary_face();
    assert_eq!(face.types, ["Creature", "Bird", "Mutant"]);
    assert_eq!((face.power, face.toughness), (Some(0), Some(2)));
    assert_food_leave_trigger(face, "featherbrained_filcher");
}

#[test]
fn issue_297_reuses_the_predefined_food_token_contract() {
    let registry = CardRegistry::global();
    let food = registry.get("food").expect("Food token");
    assert!(registry.is_token("food"));
    let face = food.primary_face();
    assert_eq!(face.types, ["Artifact", "Food"]);
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Food must have exactly one activated ability");
    };
    assert_eq!(ability.timing, ActivationTiming::Normal);
    assert_eq!(
        ability.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{2}").expect("Food generic mana cost")),
            AbilityCost::Tap,
            AbilityCost::SacrificeSelf,
        ]
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::GainLife {
            amount: Amount::Fixed(3),
        }]
    );
}
