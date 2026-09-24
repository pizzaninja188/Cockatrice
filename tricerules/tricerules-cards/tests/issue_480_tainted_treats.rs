mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    ConditionObjectRef, EffectSubject, GameCondition, PermanentTypeFilter, SpellEffectKind,
    TargetKind,
};
use tricerules_cards::CardRegistry;

#[test]
fn issue_480_registers_complete_tainted_treats() {
    let registry = CardRegistry::global();
    assert_eq!(
        registry.id_for_name("Tainted Treats"),
        Some("tainted_treats")
    );
    let face = FaceExpectation {
        id: "tainted_treats",
        name: "Tainted Treats",
        face_id: "tainted_treats",
        mana_cost: "{1}{B}{G}",
        types: &["Instant"],
        keywords: &[],
        power_toughness: None,
    }
    .check();
    let [SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(target),
    }, SpellEffectKind::Conditional { condition, effect }] = face.spell_effect.as_slice()
    else {
        panic!("destroy followed by a mana-value conditional Food effect");
    };
    assert_eq!(target.kind, TargetKind::AnyPermanent);
    assert_eq!(
        target.permanent_types,
        [PermanentTypeFilter::Artifact, PermanentTypeFilter::Creature]
    );
    assert!(matches!(
        condition,
        GameCondition::ObjectManaValue {
            object: ConditionObjectRef::ChosenTarget {
                group_index: 0,
                target_index: 0
            },
            min: None,
            max: Some(4),
        }
    ));
    assert!(matches!(
        effect.as_ref(),
        SpellEffectKind::CreateTokens { token, .. } if token == "food"
    ));
    let groups = &face.targeting.as_ref().unwrap().groups;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].min, 1);
    assert_eq!(groups[0].max, 1);
}
