use tricerules_cards::primitives::{ActivationTiming, EffectSubject, TargetController, TargetKind};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, Amount, CardRegistry, Keyword, SpellEffectKind,
    TriggerCondition,
};

#[test]
fn issue_206_map_token_has_the_exact_explore_activation() {
    let registry = CardRegistry::global();
    assert!(registry.is_token("map"));
    let map = registry.get("map").expect("Map token");
    let face = map.primary_face();
    assert_eq!(map.name, "Map");
    assert_eq!(face.types, ["Artifact", "Map"]);
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Map has one activated ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(ability.timing, ActivationTiming::SorcerySpeed);
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap, AbilityCost::SacrificeSelf]
            if cost.to_string() == "{1}"
    ));
    let [SpellEffectKind::Explore {
        subject: EffectSubject::Chosen(filter),
    }] = ability.effect.as_slice()
    else {
        panic!("Map makes one chosen creature explore");
    };
    assert_eq!(filter.kind, TargetKind::Creature);
    assert_eq!(filter.controller, TargetController::You);
}

#[test]
fn issue_206_spyglass_siren_has_flying_and_creates_one_map_on_entry() {
    let registry = CardRegistry::global();
    let siren = registry.get("spyglass_siren").expect("Spyglass Siren");
    let face = siren.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{U}");
    assert_eq!(face.types, ["Creature", "Siren", "Pirate"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(1)));
    assert_eq!(face.keywords, [Keyword::Flying]);
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Spyglass Siren has one triggered ability");
    };
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::CreateTokens {
            token,
            count: Amount::Fixed(1),
            ..
        }] if token == "map"
    ));
}
