//! Actual Oracle e4b3e0af-5caa-47c1-b5be-c56a875cbb15.
use tricerules_cards::primitives::{
    Amount, CastTriggerPlayer, CounterKind, EffectSubject, SpellEffectKind, StaticAbilityDef,
    TriggerCondition,
};
use tricerules_cards::AbilityPresentation;
#[test]
fn as_foretold_actual_identity_has_both_complete_clauses() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("as_foretold")
        .expect("actual As Foretold must be registered");
    assert_eq!(registry.id_for_name("As Foretold"), Some("as_foretold"));
    assert!(!card.commander_setup_only);
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{U}");
    assert_eq!(face.types, ["Enchantment"]);
    assert_eq!(face.triggered_abilities.len(), 1);
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(
        face.static_abilities[0].definition,
        StaticAbilityDef::AsForetoldAlternativeCost
    );
    assert_eq!(face.static_abilities[0].ability_id.as_str(), "static_01");
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    let trigger = &face.triggered_abilities[0];
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        trigger.trigger,
        TriggerCondition::AtBeginningOfUpkeep {
            player: CastTriggerPlayer::Controller
        }
    );
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::Time,
            count: Amount::Fixed(1),
            subject: EffectSubject::Source
        }]
    );
}
