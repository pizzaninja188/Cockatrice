use tricerules_cards::primitives::{
    PermanentTypeFilter, PlayerRecipient, SpellEffectKind, TargetController, TargetFilter,
    TargetKind,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, ActivationTiming, Amount, CardRegistry,
    Keyword, ManaCost,
};

#[test]
fn issue_300_registers_exactly_the_reviewed_land_sacrifice_draw_cohort() {
    let registry = CardRegistry::global();
    let ripchain = registry
        .get("ripchain_razorkin")
        .expect("Ripchain Razorkin must be registered")
        .primary_face();
    assert_eq!(
        registry.id_for_name("Ripchain Razorkin"),
        Some("ripchain_razorkin")
    );
    assert_eq!(ripchain.face_id.as_str(), "ripchain_razorkin");
    assert_eq!(ripchain.mana_cost.to_string(), "{3}{R}");
    assert_eq!(ripchain.types, ["Creature", "Human", "Berserker"]);
    assert_eq!((ripchain.power, ripchain.toughness), (Some(5), Some(3)));
    assert_eq!(ripchain.keywords, [Keyword::Reach]);
    let [ripchain_ability] = ripchain.activated_abilities.as_slice() else {
        panic!("Ripchain Razorkin must have exactly one activated ability");
    };
    assert_land_sacrifice_draw(ripchain_ability, 2);

    let monstrosaur = registry
        .get("seismic_monstrosaur")
        .expect("Seismic Monstrosaur must be registered")
        .primary_face();
    assert_eq!(
        registry.id_for_name("Seismic Monstrosaur"),
        Some("seismic_monstrosaur")
    );
    assert_eq!(monstrosaur.face_id.as_str(), "seismic_monstrosaur");
    assert_eq!(monstrosaur.mana_cost.to_string(), "{4}{R}{R}");
    assert_eq!(monstrosaur.types, ["Creature", "Dinosaur"]);
    assert_eq!(
        (monstrosaur.power, monstrosaur.toughness),
        (Some(6), Some(5))
    );
    assert_eq!(monstrosaur.keywords, [Keyword::Trample]);
    let [monstrosaur_ability, typecycling] = monstrosaur.activated_abilities.as_slice() else {
        panic!("Seismic Monstrosaur must retain its draw and Mountaincycling abilities");
    };
    assert_land_sacrifice_draw(monstrosaur_ability, 2);
    assert_eq!(typecycling.ability_id.as_str(), "activated_02");
    assert_eq!(
        typecycling.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(typecycling.source_zone, AbilitySourceZone::Hand);
    assert_eq!(
        typecycling.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{2}").unwrap()),
            AbilityCost::DiscardSelf
        ]
    );
}

fn assert_land_sacrifice_draw(ability: &tricerules_cards::ActivatedAbilityDef, line: u16) {
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![line])
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(ability.timing, ActivationTiming::Normal);
    assert_eq!(ability.cost_modifiers, []);
    assert_eq!(
        ability.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{2}{R}").unwrap()),
            AbilityCost::SacrificePermanent {
                filter: TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    controller: TargetController::You,
                    permanent_types: vec![PermanentTypeFilter::Land],
                    ..TargetFilter::default()
                }
            }
        ]
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );
    assert!(ability.targeting.is_none());
    assert!(ability.conditions.is_empty());
    assert!(ability.activation_limit.is_none());
}
