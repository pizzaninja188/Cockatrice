//! Exact production identity, Oracle-line ownership and typed roles for Luxior, Giada's Gift.

use tricerules_cards::primitives::{
    AbilityCost, CountExpression, SpellEffectKind, StaticAbilityDef, TargetController,
    TargetFilter, TargetKind, TypeLineAddition,
};
use tricerules_cards::{
    AbilityPresentation, AbilitySourceZone, ActivationTiming, ManaCost, PermanentTypeFilter,
};

const LUXIOR: &str = "luxior,_giadas_gift";

#[test]
fn luxior_has_exact_identity_and_four_oracle_mapped_abilities() {
    let registry = tricerules_cards::registry::global();
    let definition = registry
        .get(LUXIOR)
        .expect("Luxior, Giada's Gift must be admitted as a production card");

    assert_eq!(registry.id_for_name("Luxior, Giada's Gift"), Some(LUXIOR));
    assert_eq!(definition.id, LUXIOR);
    assert_eq!(definition.name, "Luxior, Giada's Gift");
    assert!(!definition.commander_setup_only);
    assert_eq!(definition.faces.len(), 1);

    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "luxior_giada_s_gift");
    assert_eq!(face.mana_cost.to_string(), "{1}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Artifact", "Equipment"]);
    assert_eq!((face.power, face.toughness), (None, None));

    let [counter_bonus, type_change] = face.static_abilities.as_slice() else {
        panic!("Luxior has two distinct static ability origins");
    };
    assert_eq!(counter_bonus.ability_id.as_str(), "static_01");
    assert_eq!(
        counter_bonus.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        &counter_bonus.definition,
        StaticAbilityDef::AttachedModifier {
            add_types: TypeLineAddition { card_types, .. },
            remove_planeswalker: false,
            set_types: None,
            delta_power: 0,
            delta_toughness: 0,
            count: Some(CountExpression::AttachedObjectCounterCount),
            power_per_match: 1,
            toughness_per_match: 1,
            set_power: None,
            set_toughness: None,
            ..
        } if card_types.is_empty()
    ));

    assert_eq!(type_change.ability_id.as_str(), "static_02");
    assert_eq!(
        type_change.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        &type_change.definition,
        StaticAbilityDef::AttachedModifier {
            add_types: TypeLineAddition { card_types, .. },
            remove_planeswalker: true,
            set_types: None,
            delta_power: 0,
            delta_toughness: 0,
            count: None,
            power_per_match: 0,
            toughness_per_match: 0,
            set_power: None,
            set_toughness: None,
            ..
        } if card_types.as_slice() == [PermanentTypeFilter::Creature]
    ));

    let [planeswalker_equip, creature_equip] = face.activated_abilities.as_slice() else {
        panic!("Luxior has exactly the two printed Equip abilities");
    };
    assert_eq!(planeswalker_equip.ability_id.as_str(), "activated_01");
    assert_eq!(
        planeswalker_equip.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        planeswalker_equip.source_zone,
        AbilitySourceZone::Battlefield
    );
    assert_eq!(planeswalker_equip.timing, ActivationTiming::Normal);
    assert_eq!(
        planeswalker_equip.costs,
        [AbilityCost::Mana(
            ManaCost::parse("{1}").expect("planeswalker Equip cost")
        )]
    );
    assert_eq!(
        planeswalker_equip.effect,
        [SpellEffectKind::EquipPlaneswalker {
            target: TargetFilter {
                kind: TargetKind::AnyPermanent,
                controller: TargetController::You,
                permanent_types: vec![PermanentTypeFilter::Planeswalker],
                ..TargetFilter::default()
            }
        }]
    );
    assert!(planeswalker_equip.is_equip());

    assert_eq!(creature_equip.ability_id.as_str(), "activated_02");
    assert_eq!(
        creature_equip.presentation,
        AbilityPresentation::OracleLines(vec![4])
    );
    assert_eq!(creature_equip.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(creature_equip.timing, ActivationTiming::Normal);
    assert_eq!(
        creature_equip.costs,
        [AbilityCost::Mana(
            ManaCost::parse("{3}").expect("ordinary Equip cost")
        )]
    );
    assert_eq!(
        creature_equip.effect,
        [SpellEffectKind::Equip {
            target: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            }
        }]
    );
    assert!(creature_equip.is_equip());
}
