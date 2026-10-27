//! Teferi, Temporal Archmage's optional four-target -1 untap ability.
use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, SpellEffectKind, TargetFilter, TargetKind,
};
use tricerules_cards::AbilityPresentation;

const TEFERI: &str = "teferi,_temporal_archmage";

#[test]
fn teferi_minus_one_is_optional_four_target_any_permanent_untap() {
    let card = tricerules_cards::registry::global()
        .get(TEFERI)
        .expect("Teferi, Temporal Archmage must be registered");
    let minus = card
        .primary_face()
        .activated_abilities
        .iter()
        .find(|ability| ability.ability_id.as_str() == "activated_02")
        .expect("Teferi has a -1 activated ability");

    assert_eq!(
        minus.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(minus.costs, [AbilityCost::Loyalty(-1)]);
    assert!(minus.program.is_none());
    let any_permanent = TargetFilter {
        kind: TargetKind::AnyPermanent,
        ..Default::default()
    };
    assert!(matches!(
        minus.effect.as_slice(),
        [SpellEffectKind::Untap {
            subject: EffectSubject::Chosen(filter)
        }] if filter.as_ref() == &any_permanent
    ));

    let [group] = minus
        .targeting
        .as_ref()
        .expect("one optional shared target group")
        .groups
        .as_slice()
    else {
        panic!("one optional shared target group");
    };
    assert_eq!((group.min, group.max), (0, 4));
    assert_eq!(group.effect_indices, [0]);
}
