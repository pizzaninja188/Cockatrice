use tricerules_cards::primitives::{
    EffectSubject, SpellEffectKind, StaticAbilityDef, TargetController, TargetFilter, TargetKind,
};
use tricerules_cards::{CardRegistry, TriggerCondition};

#[test]
fn meltstriders_resolve_has_complete_oracle_behavior() {
    let definition = CardRegistry::global()
        .get("meltstriders_resolve")
        .expect("Meltstrider's Resolve must be registered");
    let face = definition.primary_face();

    assert_eq!(definition.name, "Meltstrider's Resolve");
    assert_eq!(face.mana_cost.to_string(), "{G}");
    assert_eq!(face.types, ["Enchantment", "Aura"]);
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::AuraAttach {
            target: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            },
        }]
    );

    assert_eq!(face.triggered_abilities.len(), 1);
    let trigger = &face.triggered_abilities[0];
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::Fight {
            first: EffectSubject::AttachedObject,
            second: EffectSubject::Chosen(target),
        }] if target.kind == TargetKind::Creature
            && target.controller == TargetController::Opponent
    ));
    let targeting = trigger.targeting.as_ref().expect("optional fight target");
    assert_eq!(targeting.groups.len(), 1);
    assert_eq!(targeting.groups[0].min, 0);
    assert_eq!(targeting.groups[0].max, 1);
    assert_eq!(targeting.groups[0].effect_indices, [0]);

    assert_eq!(face.static_abilities.len(), 1);
    assert!(matches!(
        &face.static_abilities[0].definition,
        StaticAbilityDef::AttachedModifier {
            delta_power: 0,
            delta_toughness: 2,
            restriction,
            ..
        } if restriction.maximum_blockers == Some(1)
    ));
}
