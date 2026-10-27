//! Teferi, Temporal Archmage's exact identity and full-card registration.
use tricerules_cards::primitives::{
    AbilityCost, LibraryBottomOrder, SpellEffectKind, StaticEmblemEffect,
};
use tricerules_cards::AbilityPresentation;

const TEFERI: &str = "teferi,_temporal_archmage";

#[test]
fn teferi_exact_identity_and_plus_one_flat_effect_are_registered() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(
        registry.id_for_name("Teferi, Temporal Archmage"),
        Some(TEFERI)
    );
    let card = registry
        .get(TEFERI)
        .expect("Teferi, Temporal Archmage must be registered");
    assert_eq!(card.id, TEFERI);
    assert_eq!(card.name, "Teferi, Temporal Archmage");
    assert!(
        !card.commander_setup_only,
        "the full Teferi definition is admitted as an ordinary mainboard card"
    );
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "teferi_temporal_archmage");
    assert_eq!(face.mana_cost.to_string(), "{4}{U}{U}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Teferi"]);
    assert_eq!(face.loyalty, Some(5));

    let plus = face
        .activated_abilities
        .iter()
        .find(|ability| ability.ability_id.as_str() == "activated_01")
        .expect("the first loyalty ability is Teferi's +1");
    assert_eq!(plus.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(plus.costs, [AbilityCost::Loyalty(1)]);
    assert!(plus.targeting.is_none());
    assert!(
        plus.program.is_none(),
        "+1 uses the admitted flat ability effect"
    );
    assert_eq!(
        plus.effect,
        [SpellEffectKind::LookChooseToHand {
            count: 2,
            filter: None,
            min: 1,
            max: 1,
            reveal: false,
            bottom_order: LibraryBottomOrder::Chosen,
        }]
    );
}

#[test]
fn teferi_minus_ten_creates_the_planeswalker_instant_timing_emblem() {
    let card = tricerules_cards::registry::global()
        .get(TEFERI)
        .expect("Teferi, Temporal Archmage must be registered");
    let ultimate = card
        .primary_face()
        .activated_abilities
        .iter()
        .find(|ability| ability.ability_id.as_str() == "activated_03")
        .expect("the third activated ability is Teferi's -10");

    assert_eq!(
        ultimate.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(ultimate.costs, [AbilityCost::Loyalty(-10)]);
    assert!(ultimate.targeting.is_none());
    assert!(ultimate.program.is_none());
    assert!(matches!(
        ultimate.effect.as_slice(),
        [SpellEffectKind::CreateStaticEmblem { effects, .. }]
            if effects.as_slice() == [StaticEmblemEffect::PlaneswalkerLoyaltyInstantTiming]
    ));
}
