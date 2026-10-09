use tricerules_cards::primitives::{EffectContext, SpellEffectKind, TargetFilter, TargetKind};
use tricerules_cards::Color;

#[test]
fn deepglow_skate_exact_printed_identity_and_optional_permanent_target_group() {
    let card = tricerules_cards::registry::global()
        .get("deepglow_skate")
        .unwrap();
    let face = card.primary_face();
    assert_eq!(card.name, "Deepglow Skate");
    assert_eq!(face.mana_cost.to_string(), "{4}{U}");
    assert_eq!(face.types, ["Creature", "Fish"]);
    assert_eq!(face.power, Some(3));
    assert_eq!(face.toughness, Some(3));
    assert_eq!(face.colors(), [Color::Blue]);
    assert!(face.keywords.is_empty());
    assert_eq!(face.triggered_abilities.len(), 1);
    let ability = &face.triggered_abilities[0];
    let [SpellEffectKind::DoubleCounters { target }] = ability.effect.as_slice() else {
        panic!("doubling effect")
    };
    assert_eq!(target.kind, TargetKind::AnyPermanent);
    let group = &ability.targeting.as_ref().unwrap().groups[0];
    assert_eq!(group.min, 0);
    assert_eq!(group.max, u32::MAX);
    assert_eq!(group.effect_indices, [0]);
}

#[test]
fn counter_doubling_rejects_player_and_mixed_player_target_filters() {
    for kind in [
        TargetKind::AnyPlayer,
        TargetKind::OpponentPlayer,
        TargetKind::AnyTarget,
    ] {
        let target = TargetFilter {
            kind,
            ..Default::default()
        };
        assert!(SpellEffectKind::DoubleCounters { target }
            .validate(EffectContext::Ability)
            .is_err());
    }
    let target = TargetFilter {
        any_of: Some(vec![
            TargetFilter {
                kind: TargetKind::AnyPermanent,
                ..Default::default()
            },
            TargetFilter {
                kind: TargetKind::AnyPlayer,
                ..Default::default()
            },
        ]),
        ..Default::default()
    };
    assert!(SpellEffectKind::DoubleCounters { target }
        .validate(EffectContext::Ability)
        .is_err());
    assert!(SpellEffectKind::DoubleCounters {
        target: TargetFilter {
            kind: TargetKind::AnyPermanent,
            ..Default::default()
        }
    }
    .validate(EffectContext::Ability)
    .is_ok());
}
