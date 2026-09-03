use tricerules_cards::primitives::{
    ConditionObjectRef, EffectSubject, GameCondition, SpellEffectKind, TargetController,
};
use tricerules_cards::{CardRegistry, CastTriggerPlayer, CounterKind, Keyword, TriggerCondition};

#[test]
fn issue_128_cards_are_complete_registry_definitions() {
    let registry = CardRegistry::global();

    let riling = registry
        .get("riling_dawnbreaker_signaling_roar")
        .expect("Riling Dawnbreaker // Signaling Roar");
    assert_eq!(riling.faces[0].triggered_abilities.len(), 1);
    let riling_trigger = &riling.faces[0].triggered_abilities[0];
    assert_eq!(
        riling_trigger.trigger,
        TriggerCondition::AtBeginningOfCombat {
            player: CastTriggerPlayer::Controller
        }
    );
    let SpellEffectKind::PumpTarget {
        power,
        toughness,
        subject: EffectSubject::Chosen(target),
        ..
    } = &riling_trigger.effect[0]
    else {
        panic!("Riling trigger must pump one chosen creature")
    };
    assert_eq!((*power, *toughness), (1, 0));
    assert_eq!(target.controller, TargetController::You);
    assert!(target
        .excluded_objects
        .contains(&tricerules_cards::TargetObjectExclusion::Source));

    let cheerleader = registry
        .get("acrobatic_cheerleader")
        .expect("Acrobatic Cheerleader");
    let cheerleader_face = &cheerleader.faces[0];
    assert_eq!(cheerleader.name, "Acrobatic Cheerleader");
    assert_eq!(cheerleader_face.mana_cost.to_string(), "{1}{W}");
    assert_eq!(cheerleader_face.types, ["Creature", "Human", "Survivor"]);
    assert_eq!(cheerleader_face.power, Some(2));
    assert_eq!(cheerleader_face.toughness, Some(2));
    assert_eq!(cheerleader_face.triggered_abilities.len(), 1);
    let survival = &cheerleader_face.triggered_abilities[0];
    assert_eq!(
        survival.trigger,
        TriggerCondition::AtBeginningOfSecondMainPhase {
            player: CastTriggerPlayer::Controller
        }
    );
    assert_eq!(
        survival.intervening_if,
        Some(GameCondition::ObjectTapped {
            object: ConditionObjectRef::Source,
            tapped: true,
        })
    );
    assert!(survival.triggers_only_once);
    assert!(matches!(
        survival.effect.as_slice(),
        [SpellEffectKind::PutCounters {
            counter: CounterKind::Keyword(Keyword::Flying),
            count: tricerules_cards::Amount::Fixed(1),
            subject: EffectSubject::Source,
        }]
    ));
}
