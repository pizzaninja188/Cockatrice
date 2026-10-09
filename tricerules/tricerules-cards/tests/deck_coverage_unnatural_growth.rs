use tricerules_cards::primitives::{
    CastTriggerPlayer, CreatureScopeController, CreatureScopeFilter, EffectContext,
    SpellEffectKind, TargetKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, Color, Layout};

#[test]
fn growth_registers_complete_single_combat_trigger_and_exact_oracle_mapping() {
    let card = tricerules_cards::registry::global()
        .get("unnatural_growth")
        .unwrap();
    assert_eq!(card.name, "Unnatural Growth");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "unnatural_growth");
    assert_eq!(face.mana_cost.to_string(), "{1}{G}{G}{G}{G}");
    assert_eq!(face.mana_cost.mana_value(), 5);
    assert_eq!(face.types, ["Enchantment"]);
    assert!(face.supertypes.is_empty());
    assert_eq!(face.colors(), [Color::Green]);
    assert!(face.power.is_none() && face.toughness.is_none());
    assert!(face.keywords.is_empty());
    assert!(face.spell_effect.is_empty() && face.static_abilities.is_empty());
    assert!(face.activated_abilities.is_empty());
    assert_eq!(face.triggered_abilities.len(), 1);
    let trigger = &face.triggered_abilities[0];
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        trigger.trigger,
        TriggerCondition::AtBeginningOfCombat {
            player: CastTriggerPlayer::AnyPlayer
        }
    );
    assert!(!trigger.may && !trigger.triggers_only_once);
    assert!(trigger.intervening_if.is_none() && trigger.max_triggers_per_turn.is_none());
    assert!(trigger.modal.is_none() && trigger.targeting.is_none());
    assert_eq!(
        trigger.effect,
        [SpellEffectKind::DoublePowerToughnessAll {
            filter: CreatureScopeFilter {
                controller: Some(CreatureScopeController::YouControl),
                ..Default::default()
            },
        }]
    );
    assert!(trigger.effect[0].target_roles().is_empty());
}

#[test]
fn growth_instruction_validates_scope_and_shared_player_target_roles() {
    let valid = SpellEffectKind::DoublePowerToughnessAll {
        filter: CreatureScopeFilter {
            controller: Some(CreatureScopeController::TargetedPlayer {
                group_index: 7,
                kind: TargetKind::AnyPlayer,
            }),
            ..Default::default()
        },
    };
    valid.validate(EffectContext::Ability).unwrap();
    assert_eq!(valid.target_roles().len(), 1);
    let invalid_player = SpellEffectKind::DoublePowerToughnessAll {
        filter: CreatureScopeFilter {
            controller: Some(CreatureScopeController::TargetedPlayer {
                group_index: 7,
                kind: TargetKind::Creature,
            }),
            ..Default::default()
        },
    };
    assert!(invalid_player.validate(EffectContext::Ability).is_err());
    let invalid_filter = SpellEffectKind::DoublePowerToughnessAll {
        filter: CreatureScopeFilter {
            name: Some(" ".into()),
            ..Default::default()
        },
    };
    assert!(invalid_filter.validate(EffectContext::Ability).is_err());
    let encoded = ron::to_string(&valid).unwrap();
    assert_eq!(ron::from_str::<SpellEffectKind>(&encoded).unwrap(), valid);
}
