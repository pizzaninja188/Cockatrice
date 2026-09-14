use tricerules_cards::primitives::{
    PermanentTypeFilter, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, TriggerCondition};

#[test]
fn issue_273_web_up_is_registered_with_mandatory_opponent_nonland_targeting() {
    let face = CardRegistry::global().get("web_up").unwrap().primary_face();
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Web Up must have one ETB ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let [SpellEffectKind::ExileUntilSourceLeaves { target }] = ability.effect.as_slice() else {
        panic!("Web Up must use linked exile");
    };
    assert_eq!(target.kind, TargetKind::AnyPermanent);
    assert_eq!(target.controller, TargetController::Opponent);
    assert_eq!(target.excluded_permanent_types, [PermanentTypeFilter::Land]);
    let [group] = ability.targeting.as_ref().unwrap().groups.as_slice() else {
        panic!("Web Up must have one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
}
